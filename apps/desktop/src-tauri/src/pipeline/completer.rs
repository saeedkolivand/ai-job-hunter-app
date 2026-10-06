//! [`Completer`]: what it is, how it is resolved, its accounting primitives
//! (charging/recording spend), and its web-search-backed research methods
//! (company briefs, market salary ranges, per-question reference notes,
//! gated behind the shared `"ai_research"` admission bucket). The actual
//! provider-call methods live in [`super::completion`].

use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::{record_usage, resolve, AiProvider, ProviderId, Usage};
use crate::error::AppResult;

/// Binds the active provider + model + app handle so any pipeline stage can run a
/// non-streaming completion through the *centralized* provider layer — same
/// [`from_active`](Self::from_active) resolution, keychain auth, capabilities,
/// and request tracing as chat. Shared platform infrastructure, not a
/// per-feature detail.
pub struct Completer {
    pub(super) app: AppHandle,
    pub(super) provider: Box<dyn AiProvider>,
    pub(super) model: String,
    /// The resolved base URL, when one was supplied — only meaningful for
    /// `openai-compatible` (LM Studio/vLLM/OpenRouter/…). Threaded through to
    /// [`record_usage`]'s free/paid cost gate; `None` for every other
    /// provider, which the gate ignores it for.
    pub(super) base_url: Option<String>,
    /// The context window the user configured for THIS resolved model, sent as
    /// `options.num_ctx` on every request this completer builds.
    ///
    /// `None` means "the provider's own default" and is sent as nothing at all
    /// — never a guessed size. A model's real trained window is knowable only
    /// by asking the server (`ai_inspect_model` → `/api/show`), which is a
    /// network call this resolution path must not make, so the honest source is
    /// the value the user picked in Settings.
    pub(super) context_window: Option<u32>,
}

impl Completer {
    /// The `AppHandle`-free core of the store-driven [`from_config`](Self::from_config):
    /// provider present → parse → model rule → `validate_model` → construct the
    /// boxed provider client (`base_url` only honored for `OpenAiCompatible`).
    /// Extracted so it's directly unit-testable without an `AppHandle`.
    pub(super) fn resolve_parts(
        provider: Option<&str>,
        model: Option<&str>,
        base_url: Option<String>,
    ) -> AppResult<(Box<dyn AiProvider>, String, Option<String>)> {
        let provider_str = provider
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "No AI provider selected. Choose a provider in Settings → AI.".to_string()
            })?;
        let provider_id = ProviderId::parse(provider_str)?;
        let model = match model.map(str::trim).filter(|s| !s.is_empty()) {
            Some(m) => m.to_string(),
            // CLI agents may run with no explicit model — they fall back to the
            // tool's own configured default (validated leniently below).
            None if provider_id.is_cli_agent() => String::new(),
            None => {
                return Err("No model selected for the active provider."
                    .to_string()
                    .into())
            }
        };
        provider_id.validate_model(&model)?;
        Ok((resolve(provider_id, base_url.clone()), model, base_url))
    }

    /// Resolve a `Completer` from the **backend-owned** active provider store
    /// ([`crate::ai_config::AiConfigStore`]) — the ONLY provider/model/base_url
    /// resolution path. The renderer can no longer point generation at an
    /// attacker endpoint: routing comes entirely from the persisted store, which
    /// validated every value at write time.
    ///
    /// The store's config is snapshotted (owned) before this returns, so no DB lock
    /// is held across any later `.await`.
    pub fn from_active(app: &AppHandle) -> AppResult<Self> {
        let cfg = app
            .state::<crate::ai_config::AiConfigStore>()
            .active_config();
        let (provider, model, base_url, context_window) = Self::from_config(cfg)?;
        Ok(Self {
            app: app.clone(),
            provider,
            model,
            base_url,
            context_window,
        })
    }

    /// Resolve a `Completer` for ONE pipeline stage.
    ///
    /// An explicitly-set override for `stage` ([`crate::ai_config::StageOverride`])
    /// wins; ANY other outcome — no row, or a stage nobody configured — falls
    /// through to [`from_active`](Self::from_active) unchanged. There is no
    /// third behaviour: a stage is never switched to a model the user did not
    /// name, which is what lets a Settings UI *suggest* per-stage defaults
    /// without applying them.
    ///
    /// The override takes the SAME chain as the active config — `base_url`
    /// re-validated against `net::ssrf` on the egress path, then
    /// [`resolve_parts`](Self::resolve_parts) — because a store row can also
    /// come from a restored backup rather than from the settings writer. A row
    /// that fails is an `Err`, never a quiet fallback to the active provider:
    /// the run the user asked for is not the run they would get.
    pub fn from_active_for_stage(app: &AppHandle, stage: &str) -> AppResult<Self> {
        let over = app
            .state::<crate::ai_config::AiConfigStore>()
            .stage_override(stage);
        match over {
            Some(over) => Self::from_override_row(app, over),
            None => Self::from_active(app),
        }
    }

    /// Build a `Completer` from an override row the caller ALREADY holds.
    ///
    /// Split out so [`for_stages`](Self::for_stages) can resolve from its own
    /// snapshot instead of re-reading the store per stage — see the atomicity
    /// note there.
    pub(super) fn from_override_row(
        app: &AppHandle,
        over: crate::ai_config::StageOverride,
    ) -> AppResult<Self> {
        // The endpoint FOLLOWS the named provider's own stored row, read at
        // resolve time rather than snapshotted into the override — see
        // `from_override`.
        let base_url = app
            .state::<crate::ai_config::AiConfigStore>()
            .provider_base_url(&over.provider);
        let (provider, model, base_url, context_window) = Self::from_override(over, base_url)?;
        Ok(Self {
            app: app.clone(),
            provider,
            model,
            base_url,
            context_window,
        })
    }

    /// The completers one RUN needs, resolved once up front: an entry for every
    /// stage in `stages` that carries an override, and nothing else.
    ///
    /// Once, not per call, for two reasons. Resolving inside a stage would read
    /// the store mid-run, so an override edited while a 40-minute run is in
    /// flight would take effect halfway through it — a document written by two
    /// different models with nothing recording which wrote what. And the map is
    /// what the stage CACHE keys off (`QualityCtx::stage_cache_key`), so a
    /// changing answer would mean a changing key.
    ///
    /// Stages with no override are ABSENT from the map rather than mapped to a
    /// clone of the active completer — "absent means the default" is the
    /// property every reader below depends on.
    pub fn for_stages(
        app: &AppHandle,
        stages: &[&str],
    ) -> AppResult<std::collections::HashMap<String, Self>> {
        let mut overrides = app
            .state::<crate::ai_config::AiConfigStore>()
            .stage_overrides();
        let mut out = std::collections::HashMap::new();
        for stage in stages {
            // Resolved from the map READ ABOVE, not by re-reading the store per
            // stage. Membership and content then come from one snapshot, so the
            // "absent means the default" invariant below is true by
            // construction: re-reading left a window in which a clear landing
            // between the two reads put a stage in the map mapped to a clone of
            // the ACTIVE completer — the one state this doc says cannot occur.
            // Nothing observable moved today (that clone's routing identity
            // equals the default's, so no cache key changes), but "the map says
            // overridden" is exactly what a later reader would trust.
            let Some(over) = overrides.remove(*stage) else {
                continue;
            };
            out.insert((*stage).to_string(), Self::from_override_row(app, over)?);
        }
        Ok(out)
    }

    /// The `AppHandle`-free resolve seam for one stage override — the sibling of
    /// [`from_config`](Self::from_config), doing the same steps in the same
    /// order so a stage's routing cannot be validated more loosely than the
    /// active provider's.
    ///
    /// `base_url` is NOT part of the override: it is the named provider's own
    /// stored endpoint, passed in by the caller that could read it. So it
    /// FOLLOWS that provider's settings rather than snapshotting them — there
    /// is exactly one base URL per provider, the one Settings shows. A
    /// snapshot would let an override keep pointing at an endpoint the user had
    /// since changed, with no screen showing the difference.
    #[allow(clippy::type_complexity)]
    pub(super) fn from_override(
        over: crate::ai_config::StageOverride,
        base_url: Option<String>,
    ) -> AppResult<(Box<dyn AiProvider>, String, Option<String>, Option<u32>)> {
        let crate::ai_config::StageOverride {
            provider,
            model,
            context_window,
        } = over;
        if let Some(url) = base_url.as_deref() {
            crate::net::ssrf::validate_provider_base_url(url)?;
        }
        // Every stored value this row carries goes through the same gate on the
        // way OUT, not just the one that can reach a network: a hand-edited
        // window is the same threat model as a hand-edited base_url, and the
        // module doc promises both. The override's OWN window is used, never
        // the active provider's — the override names a different model, so the
        // default's window would be a wrong number rather than a missing one.
        let context_window = crate::ai_config::validate_context_window(context_window)?;
        let (provider, model, base_url) =
            Self::resolve_parts(Some(&provider), Some(&model), base_url)?;
        Ok((provider, model, base_url, context_window))
    }

    /// The `AppHandle`-free validated-resolve seam behind
    /// [`from_active`](Self::from_active): the defensive re-validate of the
    /// stored `base_url` on the egress path (the writer/seed/import all validate
    /// it, so this only ever fires on a tampered store — fail closed, never
    /// silently fall back to the default endpoint), the same defensive
    /// re-validate of the stored context window, then the
    /// [`resolve_parts`](Self::resolve_parts) steps. Takes an already-read owned
    /// [`ActiveAiConfig`](crate::ai_config::ActiveAiConfig) — no store lock, no
    /// `AppHandle` — so it's directly unit-testable.
    ///
    /// Both defensive checks live HERE rather than in the `AppHandle`-bound
    /// caller so a test can reach them: an egress guard that no test can call is
    /// a guard nobody notices the deletion of.
    #[allow(clippy::type_complexity)]
    pub(super) fn from_config(
        cfg: crate::ai_config::ActiveAiConfig,
    ) -> AppResult<(Box<dyn AiProvider>, String, Option<String>, Option<u32>)> {
        if let Some(url) = cfg.base_url.as_deref() {
            crate::net::ssrf::validate_provider_base_url(url)?;
        }
        // `num_ctx` is the one stored number whose absurd value is an
        // out-of-memory kill of the user's machine rather than a wrong answer,
        // so it takes the same gate as the base_url above, under the same
        // hand-edited-store threat model. Fail closed; never silently
        // substitute a default.
        let context_window = crate::ai_config::validate_context_window(cfg.context_window)?;
        let (provider, model, base_url) = Self::resolve_parts(
            cfg.active_provider.as_deref(),
            cfg.model.as_deref(),
            cfg.base_url,
        )?;
        Ok((provider, model, base_url, context_window))
    }

    /// The provider-call seam's error scrub: every `Err` a provider call returns
    /// through a `Completer` method passes here, so the stored key and the base
    /// URL's secrets are stripped verbatim wherever the text goes next
    /// (`emit_stream_error`, `job_fail`, the pipeline's persisted run record,
    /// logs). Variant-preserving and uncapped (see
    /// [`strip_secrets_in_place`](crate::commands::ai_provider::strip_secrets_in_place)).
    ///
    /// The key is read only on the error path (one keychain read per FAILED
    /// call): the adapters resolve their own key inside the request, so this
    /// seam never holds it on the success path. It is only compared, never
    /// logged, and not retained.
    pub(super) fn strip_secrets<T>(&self, res: AppResult<T>) -> AppResult<T> {
        res.map_err(|e| {
            let key = crate::commands::ai::get_provider_key(
                &self.app,
                self.provider.id().credential_key(),
            );
            crate::commands::ai_provider::strip_provider_secrets(
                e,
                key.as_deref(),
                self.base_url.as_deref(),
            )
        })
    }

    /// The app handle, so stages can reach managed state (caches, credentials) and
    /// emit events without threading `AppHandle` through every signature.
    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    /// The resolved provider's id — e.g. so a caller can charge the shared
    /// per-provider daily budget ([`crate::limits::Limiter::charge_provider_daily`])
    /// after resolving, without re-parsing the provider string itself.
    pub fn provider_id(&self) -> ProviderId {
        self.provider.id()
    }

    /// The resolved active model — so a caller can name it in an error message
    /// after resolving, without re-deriving it from the (no longer trusted)
    /// request. Used by `pipeline::resume::cache` to key the résumé pipeline's
    /// cache on the model it ran against.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The base URL this completer actually sends to (`openai-compatible` only).
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// The configured context window for the resolved model, for a caller that
    /// builds its own [`AiGenerateRequest`](crate::commands::ai_provider::AiGenerateRequest)
    /// rather than going through
    /// [`stream_complete`](super::completion)/`complete_json` — today the
    /// résumé pipeline's `draft` stage, which streams. `None` leaves the
    /// provider on its own default.
    pub fn context_window(&self) -> Option<u32> {
        self.context_window
    }

    /// A CHEAP reasoning-effort level for the resolved provider/model, or
    /// `None` — when the provider offers no effort LEVER at all for it (see
    /// [`AiProvider::effort_levels`](crate::commands::ai_provider::AiProvider::effort_levels)
    /// — empty is narrower than "this model doesn't reason"), AND when its
    /// lowest tier is not actually cheap. See [`low_effort_level`] for both
    /// rules and why a non-cheap lowest tier must resolve to `None` rather
    /// than to itself.
    ///
    /// For a caller whose whole job is a short, bounded answer and that has no
    /// user-chosen effort to honor: the extension bridge's `answer.assist`
    /// compose, where a reasoning model's thinking tokens are billed against
    /// the SAME `max_tokens` budget as the answer, so the cheapest tier that
    /// still produces one is the right default. Resolution is
    /// [`low_effort_level`]'s (registry-driven — a new provider/model needs
    /// no change here).
    pub fn low_effort(&self) -> Option<&'static str> {
        low_effort_level(&self.provider.effort_levels(&self.model))
    }

    /// Charge ONE provider round-trip against the shared per-provider daily
    /// ceiling — the coarse runaway-cost backstop every other fan-out
    /// chokepoint charges (`commands::pipeline`; the now-deleted agentic
    /// controller's `LiveAgentEnv::turn` and tool-call charge did the same).
    /// Fail-closed: the `Err` must abort the caller BEFORE the request is
    /// built.
    ///
    /// Resolves the managed `Arc<Limiter>` the same way every other charge
    /// site does; the limiter is managed unconditionally in `shell/state.rs`,
    /// before any command can run.
    ///
    /// `pub(crate)` for [`stream_captured`](super::completion)'s caller,
    /// for the résumé pipeline's own non-`complete_json` round-trips (the
    /// repair loop's section rewrites go through [`complete`](super::completion),
    /// which records spend but does not charge): every provider call a
    /// multi-stage run makes has to hit this ceiling, or the run is the one
    /// fan-out shape that escapes the day's cap. Also called by
    /// [`crate::cover_letter::research::CompanyResearch::enrich_with`], but
    /// ONLY after its own cache check has already come back empty — see that
    /// call site's comment for why the charge moved there instead of into
    /// [`Self::admit_research`].
    ///
    /// **Per-stage models spread the charge across buckets.** The ceiling is
    /// PER PROVIDER, and each call charges the provider THIS completer
    /// resolved — so a run whose `strategy` stage is overridden to a cloud
    /// provider charges that provider's bucket for the strategy call and the
    /// active provider's for everything else. That is the intended reading of
    /// a per-provider cap (an Ollama stage costs nothing and should not consume
    /// an OpenAI allowance), but it does mean the number of calls a single run
    /// may make grows with the number of DISTINCT providers it routes to. The
    /// per-run ceilings that bound a run's total work are `Budget::max_steps`
    /// and the `RunDeadline`, not this.
    pub(crate) fn charge_daily(&self) -> AppResult<()> {
        self.app
            .state::<std::sync::Arc<crate::limits::Limiter>>()
            .inner()
            .charge_provider_daily(
                self.provider.id().as_str(),
                crate::limits::PROVIDER_DAILY_MAX,
            )
    }

    /// Record ONE completed round-trip's REAL reported usage against today's
    /// spend. Post-call by necessity: the token counts come from the response.
    pub(super) fn record_spend(&self, usage: Usage) {
        record_usage(
            &self.app,
            self.provider.id().as_str(),
            &self.model,
            usage,
            self.base_url.as_deref(),
        );
    }

    /// Resolve the search ROUTE for a company-research pass exactly once —
    /// see `commands::ai_provider::search::CompanySearchRoute` for the full
    /// reasoning. A caller that needs the backend identity (e.g. for a cache
    /// key) MUST resolve here and reuse the result via
    /// [`research_via`](Self::research_via) — never call this a second time
    /// for the same pass and expect the two resolutions to agree:
    /// credentials can change between calls (an Exa key added/removed
    /// mid-flight), which is exactly the bug this two-phase shape closes.
    pub fn resolve_search_route(&self) -> crate::commands::ai_provider::search::CompanySearchRoute {
        crate::commands::ai_provider::search::CompanySearchRoute::resolve(
            &self.app,
            self.provider.as_ref(),
            &self.model,
        )
    }

    /// Company-research brief along an ALREADY-resolved `route` — see
    /// [`resolve_search_route`](Self::resolve_search_route). Returns `""`
    /// (never an error) when the provider can't search — see
    /// [`AiProvider::research`](crate::commands::ai_provider::AiProvider::research).
    pub async fn research_via(
        &self,
        route: crate::commands::ai_provider::search::CompanySearchRoute,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        self.strip_secrets(
            crate::commands::ai_provider::search::fetch_company_brief(
                &self.app,
                self.provider.as_ref(),
                &self.model,
                route,
                company,
                role,
            )
            .await,
        )
    }

    /// Web-grounded market salary-range lookup through the active provider's
    /// **own** web search. Returns raw (possibly noisy) text — `""` when the
    /// provider can't search — see
    /// [`AiProvider::research_salary`](crate::commands::ai_provider::AiProvider::research_salary).
    /// The caller ([`crate::salary_research::SalaryResearch`]) parses + strictly
    /// validates it before anything reaches a prompt. `country`/`currency`
    /// ground the report in the job's actual currency; both empty when unknown.
    pub async fn research_salary(
        &self,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        if self.provider.has_native_search(&self.model) {
            return self.strip_secrets(
                self.provider
                    .research_salary(
                        &self.app,
                        &self.model,
                        role,
                        company,
                        location,
                        country,
                        currency,
                    )
                    .await,
            );
        }
        self.strip_secrets(
            crate::commands::ai_provider::search::searched_research_salary(
                &self.app,
                self.provider.as_ref(),
                &self.model,
                role,
                company,
                location,
                country,
                currency,
            )
            .await,
        )
    }

    /// Web-search reference notes for a single application-question answer
    /// through the active provider's **own** web search — the per-question
    /// sibling of the company-brief research family
    /// ([`resolve_search_route`](Self::resolve_search_route)/
    /// [`research_via`](Self::research_via)). Returns `""` (never an error)
    /// when the provider can't search — see
    /// [`AiProvider::research_answer`](crate::commands::ai_provider::AiProvider::research_answer).
    pub async fn research_answer(
        &self,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        if self.provider.has_native_search(&self.model) {
            return self.strip_secrets(
                self.provider
                    .research_answer(&self.app, &self.model, question, role, company)
                    .await,
            );
        }
        self.strip_secrets(
            crate::commands::ai_provider::search::searched_research_answer(
                &self.app,
                self.provider.as_ref(),
                &self.model,
                question,
                role,
                company,
            )
            .await,
        )
    }

    /// Whether company research can actually run right now — a configured search
    /// backend exists, not merely a provider that advertises one. See
    /// `ai_provider::search::research_available`.
    pub fn research_available(&self) -> bool {
        crate::commands::ai_provider::search::research_available(
            &self.app,
            self.provider.as_ref(),
            &self.model,
        )
    }

    /// Admit one billable, provider-web-search company-research call against
    /// the shared [`crate::limits::AI_RESEARCH_BUCKET`] rate + concurrency
    /// cap — the SAME bucket `commands::ai::admit_research` gates
    /// `ai_lookup_salary`/`ai_research_answer` behind, and (as of the
    /// cache-hit-charges-the-daily-budget fix) the one `ai_research_company`
    /// admits into too, via this exact method — the two callers that share
    /// [`crate::cover_letter::research::CompanyResearch::enrich_with`] now
    /// also share ONE admission implementation instead of two that could
    /// silently drift.
    ///
    /// Deliberately charges **no daily budget** here — only the rate +
    /// concurrency slot, which exists to bound a stampede of *concurrent*
    /// requests regardless of whether any of them turns out to need a real
    /// provider call. The per-provider daily ceiling
    /// ([`Self::charge_daily`]) is charged by `enrich_with` itself, ONLY
    /// once it has already checked its cache and confirmed a real provider
    /// request is about to fire — a cache hit, or a request with no
    /// resolvable company name, must never spend a day's allowance. Charging
    /// eagerly here (the pre-fix shape) burned the daily ceiling on every
    /// cache hit, silently shrinking the effective allowance to whatever
    /// fraction of calls actually missed the cache.
    ///
    /// Returns `None` on rate-limit/concurrency refusal — the résumé
    /// pipeline (and `ai_research_company`) have no renderer-facing
    /// distinction to make there the way `commands::ai::AdmitOutcome` does
    /// for `ai_lookup_salary`: a refusal always means "skip the research,"
    /// never a failed run. `who` only labels the debug log.
    pub(crate) fn admit_research(&self, who: &str) -> Option<crate::limits::ConcurrencyGuard> {
        let limiter = self
            .app
            .state::<std::sync::Arc<crate::limits::Limiter>>()
            .inner()
            .clone();
        match limiter.acquire(
            crate::limits::AI_RESEARCH_BUCKET,
            crate::limits::AI_RESEARCH_RATE_MAX,
            crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
        ) {
            Ok(g) => Some(g),
            Err(e) => {
                tracing::debug!("{who}: research rate limited: {e}");
                None
            }
        }
    }
}

/// A CHEAP tier from a provider's own
/// [`effort_levels`](crate::commands::ai_provider::AiProvider::effort_levels)
/// list for a model: entry 0, but ONLY when entry 0 is `minimal` or `low`.
/// `None` otherwise — for the empty list (the provider exposes no effort
/// lever for it, so there is nothing to send and nothing to invent) and for a
/// list whose own lowest tier is already an expensive one.
///
/// Two rules, both load-bearing:
///
/// * **Entry 0, not a name match.** The lowest tier is not always spelled
///   `"low"` — Gemini's per-model lists start at `"minimal"` on some models
///   — so matching the literal would miss them. The lists are NOT sorted end
///   to end (Anthropic's finishes `"max", "xhigh"`, following its own docs'
///   enumeration rather than tier order); the invariant this rests on is only
///   that entry 0 is the MINIMUM of the list, which is pinned by
///   `crate::commands::ai_provider::tests::every_providers_effort_levels_list_its_lowest_tier_first`
///   against the live tables.
/// * **Only `minimal`/`low` are cheap.** A model whose lowest — sometimes
///   only — accepted tier is `"high"` resolves to `None`, leaving the request
///   byte-for-byte as it was before an effort was passed at all. Sending
///   `"high"` there would invert the one caller's intent twice: it asks for
///   MORE thinking (the tokens that caller is trying to keep out of its
///   output budget), and `timeouts::stream_deadline` stretches the stream's
///   deadline past the baseline for it, holding an `ai_research` concurrency
///   slot longer for a request that wanted to be cheap and short.
///
/// Free function (not just [`Completer::low_effort`]) so the choice is
/// unit-testable against the REAL provider adapters' level lists without a
/// live `AppHandle` to build a `Completer` from.
pub(crate) fn low_effort_level(levels: &[&'static str]) -> Option<&'static str> {
    levels
        .first()
        .copied()
        .filter(|level| matches!(*level, "minimal" | "low"))
}
