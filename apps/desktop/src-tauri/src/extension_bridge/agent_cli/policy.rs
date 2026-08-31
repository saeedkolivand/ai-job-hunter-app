//! ADR-038 §1 — the command policy table: every one of the 164
//! `#[tauri::command]` sites registered in `tauri::generate_handler!`
//! (`lib.rs`), classified by [`Effect`]. Phase 1 (this table) shipped with
//! nothing dispatching through it; Phase 2 (`super::super::agent_call`) reads
//! it to drive `agent call <ns>:<command>` — [`Effect::Read`] AND
//! [`Effect::Reversible`] rows dispatch directly (Phase 4), and
//! [`Effect::Irreversible`] rows dispatch only after a `--confirm` ceremony
//! whose expected value is named per-row by [`ProofSource`] (Phase 3) — but
//! ONLY [`Effect::Read`] rows are curated-tier-eligible; the value here is
//! the exactness test at the bottom: it is ADR-014's
//! (`docs/knowledge/decision-records/adr-014-cli-agent-shell-plugin-static-
//! allowlist.md`) static-allowlist invariant applied to *inbound* dispatch,
//! so a new command that lands in `generate_handler!` without a row here
//! fails CI instead of shipping silently reachable by a future caller.
//!
//! ## How each row was classified
//! - Read from the command's BODY, never its name — `*_remove` is not
//!   automatically [`Effect::Irreversible`] and `*_list` is not
//!   automatically [`Effect::Read`]; a command that returns data but also
//!   writes a cache/backfill on the way is [`Effect::Reversible`] (or
//!   [`Effect::Irreversible`]) rather than [`Effect::Read`] by default. A
//!   command whose BODY is an unimplemented stub (a hardcoded success, a
//!   bare `null`) is [`Effect::NotExposed`] even though nothing it does
//!   mutates state — `Read` promises the RETURNED DATA is real, and a stub
//!   dispatched by name would hand back a convincing lie instead. The same
//!   rule caught a THIRD case once Phase 4 made `Reversible` dispatch: `Read`
//!   also promises the returned data cost nothing to produce — `ai_embed`
//!   mutates no state (a legitimate `Read` call on that axis alone) but
//!   hits a paid embedding provider with no `charge_provider_daily`/
//!   `limiter.acquire` gate anywhere in its call chain (verified against
//!   `commands::ai::ai_embed` → `documents::embed` → `embed_text`), so
//!   dispatching it by name would let a caller spend against a paid
//!   provider with zero budget enforcement — `NotExposed` until that gate
//!   exists (a separate change: the gap is pre-existing and UI-reachable
//!   too, and the right cap is per-request or per-byte, not this table's
//!   concern).
//! - Pessimistic default: anything not fully verified from the body is
//!   [`Effect::Irreversible`].
//! - [`Effect::NotExposed`] always carries a real, specific reason — never
//!   "unclear". The genuine cases found: a native OS dialog handle with no
//!   argv/JSON equivalent (`tauri_plugin_dialog`'s blocking pickers); a
//!   window/menu/tray action that is meaningless off a UI a non-interactive
//!   caller cannot see (opens devtools, delivers a buffered intent meant
//!   for the renderer's own window, focuses the app); an unimplemented
//!   stub whose payload would misrepresent itself as real (`ai_unload_model`,
//!   `support_get_system_info` — both reclassified from `Read` once Phase 2
//!   made that classification reachable, not merely descriptive); or a real
//!   read with no anti-abuse gate on its own paid egress (`ai_embed`,
//!   reclassified once Phase 4 made the SAME thing true of its blast radius).
//!
//! ADR-038 itself names four canonical [`Effect::Irreversible`] patterns:
//! `privacy:reset_app`, `sign_out_all`, `credentials:*`, and "the `*_remove`
//! family". Applied consistently here, [`Effect::Irreversible`] covers any
//! command that: deletes or unconditionally clears persisted user data
//! (never re-derivable with the same content — a scrape/re-embed recreates
//! *some* data, not the SAME data); writes or removes a secret in the OS
//! keychain (`CredentialStore`, or `email_watch`'s IMAP app-password slot);
//! revokes a session/pairing token; spends against a paid AI provider's
//! per-day ceiling (`Limiter::charge_provider_daily`, verified per call
//! site — not every AI-adjacent command charges it); launches an external
//! program/process the app does not control; or replaces + force-restarts
//! the app binary itself. A command that clears only a *recomputable
//! derived* cache (embeddings, match scores) with no user-authored content
//! lost is [`Effect::Reversible`], not Irreversible — noted per-row where
//! that distinction is load-bearing.
//!
//! ## Phase 3 — [`ProofSource`], the confirmation ceremony's proof
//! Every `Irreversible` row now carries a [`ProofSource`]: WHERE the
//! `--confirm` value a caller must supply comes from. The value is never
//! derivable, invented, or handed out by the dispatcher itself (never a
//! hash/nonce) — it is always the user's OWN data, read fresh through
//! ANOTHER `Effect::Read` row (asserted by
//! `every_proof_source_read_command_is_a_read_row` below), so possessing it
//! proves the caller actually read the affected record. Two shapes:
//! - **A real record exists** (a delete-by-id, a run-by-id): the proof is
//!   that record's own name/title, read by the SAME id the caller supplied
//!   — `documents_remove`, `autopilot_remove`, `applications_delete`, etc.
//! - **No record exists** (a global wipe, a credential set with nothing to
//!   compare against, a caller-controlled external URL): the strongest
//!   available signal is used and the row says so — a count of what's about
//!   to be lost (`notifications_clear_all`, `privacy_clear_interactions`),
//!   or, honestly, a WEAK fallback with no real binding to the specific
//!   target (`system_open_external`, `updater_install`,
//!   `extension_bridge_regenerate_token`) — flagged per-row rather than
//!   dressed up as strict.
//!
//! Four commands here carry zero renderer references (never called from the
//! UI, per ADR-038's own Context section) — flagged per-row below: `boards::
//! boards_list`, `privacy::privacy_clear_data`,
//! `support::support_get_system_info`, `resume::extract_resume`. One of the
//! four (`privacy_clear_data`) is destructive.

// `POLICY`/`Effect`/`PolicyEntry` are now consumed by ADR-038 §2's
// `agent_call` dispatcher (`super::super::agent_call`) — kept for the odd
// field a future row might carry unread by any current match arm, mirroring
// the same allow every other exhaustively-matched policy-style table in
// this crate carries defensively.
#![allow(dead_code)]

/// The declared consequence class for one registered command (ADR-038 §1).
/// Declared, never inferred, and pessimistic by default — see the module
/// doc for the exact rule each variant follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// No state change, and no un-metered cost — the command only returns
    /// data (a network read counts as `Read` too, as long as nothing
    /// persisted changes AND nothing billable/rate-limited is spent
    /// un-gated; see `ai_embed`'s reclassification in the module doc).
    Read,
    /// Mutates persisted or in-memory state, but the change can be undone
    /// through another call on this same surface (edit again, toggle back,
    /// re-scrape, reconnect) — or clears only a recomputable derived cache
    /// with no user-authored content lost.
    Reversible,
    /// Cannot be undone through the app. See the module doc's "ADR-038
    /// itself names four canonical patterns" paragraph for exactly what
    /// qualifies. Carries the [`ProofSource`] the Phase 3 confirmation
    /// ceremony resolves its `--confirm` value against.
    Irreversible(ProofSource),
    /// Deliberately unreachable from this CLI surface. The `&'static str`
    /// is the reason a future dispatcher must refuse this command outright
    /// — never a placeholder like "unclear"
    /// (see `not_exposed_rows_carry_a_real_reason`).
    NotExposed(&'static str),
}

/// Where an [`Effect::Irreversible`] command's `--confirm` value must come
/// from (ADR-038 §4, Phase 3) — resolved FRESH at confirm-check time by
/// dispatching `read_command` through the exact same real-command path
/// every other row already uses (`agent_call::invoke_command`), never a
/// second implementation of that command's logic and never a value the
/// dispatcher invents (no hash, no nonce). Every field is `&'static`, so the
/// whole table stays `'static` data like every other row in [`POLICY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProofSource {
    /// `read_command` takes no input; the proof is the response at `path`
    /// (an empty `path` uses the bare response value itself, e.g. a plain
    /// string like `system_get_version`'s).
    Scalar {
        read_command: &'static str,
        path: &'static [&'static str],
    },
    /// `read_command` takes ONE input key, `key`, whose value is either
    /// forwarded verbatim from the irreversible command's OWN input (the
    /// caller already supplied it to target this exact record) or a fixed
    /// literal (a selector-less command with no id of its own to forward).
    /// The proof is the response at `path`.
    Lookup {
        read_command: &'static str,
        key: &'static str,
        input: LookupInput,
        path: &'static [&'static str],
    },
    /// `read_command` takes no input and returns an ARRAY; the proof is
    /// `value_field` off the element whose `match_field` equals the
    /// irreversible command's own `id_field` input — the "delete by id,
    /// prove you read its name" shape.
    ListMatch {
        read_command: &'static str,
        id_field: &'static str,
        match_field: &'static str,
        value_field: &'static str,
    },
    /// `read_command` takes no input and returns an array; the proof is its
    /// length — the strongest available signal for a selector-less wipe
    /// (the module doc's "no record exists" case).
    Count { read_command: &'static str },
    /// `read_command` takes no input and returns an array; the proof is the
    /// count of its elements whose `match_field` is a member of the
    /// irreversible command's own `ids_field` (a JSON array input) —
    /// `ai_generations_remove_bulk`'s own bulk-selector shape.
    MatchCount {
        read_command: &'static str,
        ids_field: &'static str,
        match_field: &'static str,
    },
}

impl ProofSource {
    /// The bare command name the proof is read from — every variant carries
    /// exactly one.
    pub(crate) fn read_command(self) -> &'static str {
        match self {
            ProofSource::Scalar { read_command, .. }
            | ProofSource::Lookup { read_command, .. }
            | ProofSource::ListMatch { read_command, .. }
            | ProofSource::Count { read_command }
            | ProofSource::MatchCount { read_command, .. } => read_command,
        }
    }
}

/// How [`ProofSource::Lookup`] builds the ONE input key it sends to its own
/// `read_command`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookupInput {
    /// Forward the irreversible command's own input field of this name,
    /// verbatim, as `read_command`'s SAME-named input field — the caller
    /// already supplied it to target this exact record.
    FromCaller(&'static str),
    /// A fixed literal — used only by a selector-less command that still
    /// needs ONE representative id to query (see `privacy_sign_out_all`'s
    /// row comment for why this is one of the weaker rows).
    Literal(&'static str),
}

/// One row of the policy table: the exact path `generate_handler!`
/// registers (matched verbatim against `lib.rs` by this module's own
/// exactness test), plus its declared [`Effect`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct PolicyEntry {
    /// The fully-qualified path exactly as it appears inside
    /// `tauri::generate_handler![...]` in `lib.rs`, e.g.
    /// `"commands::jobs::jobs_list"`.
    pub(crate) path: &'static str,
    pub(crate) effect: Effect,
}

/// Every registered command, grouped by source module in the same order
/// `lib.rs`'s `generate_handler!` list uses (so the two are easy to diff by
/// eye, not just by the test below).
pub(crate) const POLICY: &[PolicyEntry] = &[
    // commands/cli_agents.rs
    PolicyEntry { path: "commands::cli_agents::cli_agents_status", effect: Effect::Read },
    // Clears + re-probes an in-process detection cache only (no persisted
    // write) — self-heals on the very next status call.
    PolicyEntry { path: "commands::cli_agents::cli_agents_redetect", effect: Effect::Reversible },

    // commands/system/mod.rs
    PolicyEntry { path: "commands::system::system_health", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_get_version", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_get_locale", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_set_locale", effect: Effect::Reversible },
    PolicyEntry { path: "commands::system::system_get_platform", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_accent_color", effect: Effect::Read },
    // Launches the OS's default http(s) handler (an external process this
    // app does not control) — scheme-allowlisted, but still an external
    // side effect with no undo, per the module doc's pattern list. No
    // record exists to prove a caller read (the url IS the caller's own
    // input, so echoing it back would prove nothing) — the current app
    // version is the strongest available unrelated signal; WEAK, flagged.
    PolicyEntry {
        path: "commands::system::system_open_external",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        }),
    },
    PolicyEntry { path: "commands::system::system_set_performance_mode", effect: Effect::Reversible },
    PolicyEntry { path: "commands::system::system_get_launch_at_login", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_set_launch_at_login", effect: Effect::Reversible },
    PolicyEntry { path: "commands::system::system_set_close_to_tray", effect: Effect::Reversible },
    PolicyEntry { path: "commands::system::system_get_metrics", effect: Effect::Read },
    PolicyEntry { path: "commands::system::system_check_browser", effect: Effect::Read },
    PolicyEntry {
        path: "commands::system::system_open_devtools",
        effect: Effect::NotExposed(
            "opens a debugging devtools window on the app's own webview; meaningless for a \
             non-interactive caller with no window to look at, and nothing is returned",
        ),
    },
    PolicyEntry { path: "commands::system::system_get_protocol_version", effect: Effect::Read },

    // commands/menu.rs
    PolicyEntry {
        path: "commands::menu::menu_take_pending",
        effect: Effect::NotExposed(
            "atomically consumes a buffered native-menu-click intent meant for the renderer's \
             own window; a CLI call would steal it out from under the real UI, which would then \
             silently drop the click",
        ),
    },

    // commands/jobs.rs
    PolicyEntry { path: "commands::jobs::jobs_list", effect: Effect::Read },
    PolicyEntry { path: "commands::jobs::jobs_get", effect: Effect::Read },
    PolicyEntry { path: "commands::jobs::jobs_cancel", effect: Effect::Reversible },
    // Verified: returns the job's kind/id for the RENDERER to re-dispatch —
    // does not itself restart anything, so it mutates nothing.
    PolicyEntry { path: "commands::jobs::jobs_retry", effect: Effect::Read },

    // commands/ai/mod.rs
    // Charges `Limiter::charge_provider_daily` (verified at the call site)
    // before streaming a completion — real spend against a paid provider,
    // no refund path. No id-scoped record to read back (the request is a
    // bare messages array) — the strongest available signal is the
    // caller's OWN today-so-far spend, read fresh via `ai_spend_summary`;
    // WEAK (not scoped to this specific call), flagged.
    PolicyEntry {
        path: "commands::ai::ai_generate",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    PolicyEntry { path: "commands::ai::ai_list_models", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_model_capabilities", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_inspect_model", effect: Effect::Read },
    // Charges the daily provider ceiling via `admit_research`. Same
    // no-id-to-scope-to reasoning and WEAK spend-total proof as `ai_generate`.
    PolicyEntry {
        path: "commands::ai::ai_research_company",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Charges the daily provider ceiling directly (fans out per selected question).
    PolicyEntry {
        path: "commands::ai::ai_research_answer",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Charges the daily provider ceiling via `ai_salary::ai_lookup_salary_reasoned` → `admit_research`.
    PolicyEntry {
        path: "commands::ai::ai_lookup_salary",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Downloads a model into the local Ollama store — additive, no data
    // destroyed, not charged against the paid-provider ceiling (Ollama is
    // local/free).
    PolicyEntry { path: "commands::ai::ai_pull_model", effect: Effect::Reversible },
    // ADR-038 §2 revision (Phase 2 landed dispatch-by-name): the current
    // body is a no-op stub (`_model` unused, always returns
    // `{ success: true }`) — classifying this Read would let `agent call`
    // dispatch it and hand back a CONVINCING FALSE SUCCESS for a model that
    // was never actually unloaded. `Read` truthfully describes "no state
    // change", but truthfulness about the STATE CHANGE is not the same
    // guarantee as truthfulness about the RETURNED PAYLOAD once a caller can
    // invoke this by name — NotExposed until the body is real.
    PolicyEntry {
        path: "commands::ai::ai_unload_model",
        effect: Effect::NotExposed(
            "stub — the body ignores its argument and always returns a hardcoded success; \
             dispatching it by name would hand back a convincing false success for a model \
             that was never actually unloaded",
        ),
    },
    // ADR-038 §4 revision (Phase 4 revision — security review on this PR):
    // reclassified from `Read`. `Read` promises no state change AND no
    // un-metered cost; this body (`documents::embed` → `embed_text`) hits a
    // PAID embedding provider (OpenAI/Gemini) with NO
    // `charge_provider_daily`/`limiter.acquire` anywhere in its call
    // chain — unlike `ai_generate`, which has both (verified by reading
    // both bodies, not generalized from one to the other). Dispatching this
    // by name would let a caller spend against the paid provider with zero
    // budget enforcement. Fixing the gap itself is a separate change (it is
    // pre-existing and also reachable from the UI's bulk-indexing path,
    // which embeds per chunk — a per-request daily cap may be the wrong
    // granularity there) — NotExposed until that gate exists.
    PolicyEntry {
        path: "commands::ai::ai_embed",
        effect: Effect::NotExposed(
            "no charge_provider_daily/limiter gate anywhere in this command's call chain \
             (verified: ai_embed → documents::embed → embed_text hits the paid provider \
             directly) — dispatching it by name would let a caller spend against a paid \
             embedding provider with no daily-budget cap; the gap is pre-existing and also \
             reachable from the UI, so it is fixed there, not here",
        ),
    },
    // Writes a secret into the OS keychain — ADR-038's `credentials:*` pattern.
    // No prior key to name (a fresh set has nothing to compare against); the
    // strongest available signal is whether a key for this SAME provider
    // already exists — WEAK (boolean, low entropy), flagged.
    PolicyEntry {
        path: "commands::ai::ai_set_provider_key",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "ai_has_provider_key",
            key: "provider",
            input: LookupInput::FromCaller("provider"),
            path: &["has"],
        }),
    },
    // Removes a secret from the OS keychain — `credentials:*`. Same WEAK
    // boolean proof as `ai_set_provider_key` (a real removal, `has` should
    // read `true` beforehand — but that's a coin flip's worth of entropy).
    PolicyEntry {
        path: "commands::ai::ai_remove_provider_key",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "ai_has_provider_key",
            key: "provider",
            input: LookupInput::FromCaller("provider"),
            path: &["has"],
        }),
    },
    PolicyEntry { path: "commands::ai::ai_has_provider_key", effect: Effect::Read },
    // Verified: only probes the provider (`test_key`); no store write.
    PolicyEntry { path: "commands::ai::ai_test_provider_key", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_list_provider_models", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_embedding_status", effect: Effect::Read },
    // Also clears the posting-vector/match-score caches on a real space
    // change (verified) — both are recomputable derived data, not
    // user-authored content, so this stays Reversible rather than
    // Irreversible (contrast `scrape_clear_postings` below).
    PolicyEntry { path: "commands::ai::ai_set_embedding_config", effect: Effect::Reversible },
    // Bulk re-embeds every document; charges the daily provider ceiling per
    // document. Global (no id), WEAK spend-total fallback, flagged.
    PolicyEntry {
        path: "commands::ai::ai_reembed_all",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    // Same charged re-embed path, scoped to stale documents. Same WEAK
    // spend-total fallback.
    PolicyEntry {
        path: "commands::ai::ai_index_stale_documents",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },
    PolicyEntry { path: "commands::ai::ai_spend_summary", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_active_config", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_set_active_provider", effect: Effect::Reversible },
    PolicyEntry { path: "commands::ai::ai_set_provider_settings", effect: Effect::Reversible },
    // One-time seed, row-presence gated (no-ops once anything is set).
    PolicyEntry { path: "commands::ai::ai_seed_active_config", effect: Effect::Reversible },
    PolicyEntry { path: "commands::ai::ai_stage_overrides", effect: Effect::Read },
    PolicyEntry { path: "commands::ai::ai_set_stage_override", effect: Effect::Reversible },
    PolicyEntry { path: "commands::ai::ai_clear_stage_override", effect: Effect::Reversible },

    // commands/pipeline.rs
    // Same charged-generation path as `ai_generate` (verified: charges
    // `charge_provider_daily` after admission). Same no-id / WEAK spend-total
    // fallback.
    PolicyEntry {
        path: "commands::pipeline::generate_pipeline",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
    },

    // commands/resume.rs
    // Zero renderer references (ADR-038 Context) — still a plain file-text
    // extraction with no persistence.
    PolicyEntry { path: "commands::resume::extract_resume", effect: Effect::Read },
    PolicyEntry { path: "commands::resume::resume_validate_content", effect: Effect::Read },

    // commands/resume_pipeline/mod.rs
    // Multi-stage AI-driven résumé/cover-letter generation; charges provider
    // spend. Scoped to a real résumé DOCUMENT (`resumeId`) — proof is that
    // document's own `name`, read via `documents_list`, matched by id.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_run",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "resumeId",
            match_field: "id",
            value_field: "name",
        }),
    },
    PolicyEntry { path: "commands::resume_pipeline::resume_pipeline_get", effect: Effect::Read },
    PolicyEntry { path: "commands::resume_pipeline::resume_pipeline_list_for_job", effect: Effect::Read },
    // Same charged AI-regenerate path as `resume_pipeline_run`, scoped to a
    // real run (`runId`) — proof is that run's own `jobUrl`, read via
    // `resume_pipeline_get`.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_regenerate_section",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "resume_pipeline_get",
            key: "runId",
            input: LookupInput::FromCaller("runId"),
            path: &["jobUrl"],
        }),
    },
    // Records a keep/remove verdict on the saved quality report — a
    // decision that can be re-recorded, nothing deleted.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_resolve_fabrication",
        effect: Effect::Reversible,
    },

    // commands/documents.rs
    PolicyEntry { path: "commands::documents::documents_list", effect: Effect::Read },
    PolicyEntry { path: "commands::documents::documents_import", effect: Effect::Reversible },
    PolicyEntry { path: "commands::documents::documents_recommend_template", effect: Effect::Read },
    // Deletes a stored document's extracted text permanently — no undo.
    // Proof is the target document's own `name`, read via `documents_list`.
    PolicyEntry {
        path: "commands::documents::documents_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "id",
            match_field: "id",
            value_field: "name",
        }),
    },
    PolicyEntry { path: "commands::documents::documents_set_default", effect: Effect::Reversible },
    PolicyEntry { path: "commands::documents::documents_get_text", effect: Effect::Read },

    // commands/job_preferences.rs
    PolicyEntry { path: "commands::job_preferences::job_preferences_get", effect: Effect::Read },
    PolicyEntry { path: "commands::job_preferences::job_preferences_set", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_salary_expectation",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_semantic_scoring",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::job_preferences::job_preferences_set_extra_agency_companies",
        effect: Effect::Reversible,
    },

    // commands/contact_profile.rs
    PolicyEntry { path: "commands::contact_profile::contact_profile_get", effect: Effect::Read },
    PolicyEntry { path: "commands::contact_profile::contact_profile_set", effect: Effect::Reversible },
    PolicyEntry { path: "commands::contact_profile::contact_profile_header_line", effect: Effect::Read },

    // commands/scrape.rs
    // Writes scraped postings into the shared cache — additive/recomputable.
    PolicyEntry { path: "commands::scrape::scrape_boards", effect: Effect::Reversible },
    PolicyEntry { path: "commands::scrape::scrape_url", effect: Effect::Reversible },
    // Verified: synchronous fetch/return only, no persistence.
    PolicyEntry { path: "commands::scrape::scrape_resolve_url", effect: Effect::Read },
    PolicyEntry { path: "commands::scrape::scrape_update_description", effect: Effect::Reversible },
    PolicyEntry { path: "commands::scrape::scrape_persist_job", effect: Effect::Reversible },
    // Verified: the code's OWN doc comment calls this "the real undo for
    // scrape_persist_job" — a paired toggle on the same (jobId,
    // interactionType) key, not a destructive delete.
    PolicyEntry { path: "commands::scrape::scrape_remove_interaction", effect: Effect::Reversible },
    PolicyEntry { path: "commands::scrape::scrape_list_postings", effect: Effect::Read },
    // Unconditional wipe of EVERY live posting — no selector, matches the
    // module doc's "any selector that can expand to everything" rule. Proof
    // is the exact count about to be lost, read via `scrape_list_postings`
    // itself.
    PolicyEntry {
        path: "commands::scrape::scrape_clear_postings",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "scrape_list_postings",
        }),
    },
    PolicyEntry { path: "commands::scrape::scrape_list_interactions", effect: Effect::Read },

    // commands/data.rs
    PolicyEntry {
        path: "commands::data::data_export",
        effect: Effect::NotExposed(
            "blocks on a native OS save-file dialog (tauri_plugin_dialog::blocking_save_file); \
             no argv/JSON equivalent for a non-interactive caller, and the call would hang \
             waiting on a user gesture that never comes",
        ),
    },
    PolicyEntry {
        path: "commands::data::data_import",
        effect: Effect::NotExposed(
            "blocks on a native OS file-picker dialog (tauri_plugin_dialog::blocking_pick_file); \
             same reasoning as data_export",
        ),
    },

    // commands/dedup.rs
    PolicyEntry { path: "commands::dedup::dedup_mark_not_duplicate", effect: Effect::Reversible },

    // commands/discovery.rs
    PolicyEntry { path: "commands::discovery::discovery_search_companies", effect: Effect::Read },
    PolicyEntry { path: "commands::discovery::discovery_set_starred", effect: Effect::Reversible },
    PolicyEntry { path: "commands::discovery::discovery_watched", effect: Effect::Read },

    // commands/match_resume.rs
    // Upserts a derived match-score cache row — recomputable, keyed by (resume, job).
    PolicyEntry { path: "commands::match_resume::match_resume", effect: Effect::Reversible },
    PolicyEntry { path: "commands::match_resume::match_resume_text", effect: Effect::Reversible },
    PolicyEntry { path: "commands::match_resume::resume_extract_text", effect: Effect::Read },
    PolicyEntry { path: "commands::match_resume::resume_trim_suggestions", effect: Effect::Read },

    // commands/credentials.rs
    PolicyEntry { path: "commands::credentials::credentials_available", effect: Effect::Read },

    // commands/boards.rs
    PolicyEntry {
        path: "commands::boards::boards_login_with_browser",
        effect: Effect::NotExposed(
            "drives an interactive browser login flow the user must complete by hand in an \
             opened window; there is nothing for a non-interactive caller to do once the \
             browser opens, and the call blocks on human input",
        ),
    },
    PolicyEntry { path: "commands::boards::boards_import_cookies", effect: Effect::Reversible },
    PolicyEntry { path: "commands::boards::boards_logout", effect: Effect::Reversible },
    PolicyEntry { path: "commands::boards::boards_get_status", effect: Effect::Read },
    // Zero renderer references (ADR-038 Context) — a plain status list, no mutation.
    PolicyEntry { path: "commands::boards::boards_list", effect: Effect::Read },
    PolicyEntry { path: "commands::boards::boards_catalog", effect: Effect::Read },
    PolicyEntry { path: "commands::boards::boards_health", effect: Effect::Read },

    // commands/privacy.rs
    // Zero renderer references (ADR-038 Context) — and the ONE destructive
    // command among the four zero-UI commands: disconnects 4 boards and
    // unconditionally clears the entire postings + interactions cache. No
    // single Read row captures the FULL blast radius (boards + postings +
    // interactions) — `scrape_list_postings`'s count is the strongest single
    // available signal, but it is PARTIAL; flagged.
    PolicyEntry {
        path: "commands::privacy::privacy_clear_data",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "scrape_list_postings",
        }),
    },
    // Unconditional wipe of every interaction (viewed/applied/saved) — no
    // selector, real user history lost. Proof is the EXACT count about to be
    // lost, read via `scrape_list_interactions` — precisely scoped.
    PolicyEntry {
        path: "commands::privacy::privacy_clear_interactions",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "scrape_list_interactions",
        }),
    },
    // ADR-038's own named example of Irreversible ("sign_out_all"). No
    // single Read row reports "how many of the 4 boards are connected right
    // now" — the strongest available is whether ONE representative board
    // (linkedin) currently has a session; WEAK (boolean, covers 1 of 4),
    // flagged.
    PolicyEntry {
        path: "commands::privacy::privacy_sign_out_all",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "boards_get_status",
            key: "boardId",
            input: LookupInput::Literal("linkedin"),
            path: &["connected"],
        }),
    },
    // ADR-038's own named example of Irreversible ("privacy:reset_app") —
    // full factory reset. No Read row captures the full blast radius (every
    // registered store); the count of saved AI generations is real, honest,
    // and readable, but covers only ONE of the many stores this wipes — the
    // WEAKEST row in this table alongside `updater_install`, flagged
    // prominently.
    PolicyEntry {
        path: "commands::privacy::privacy_reset_app",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "ai_generations_list",
        }),
    },
    PolicyEntry { path: "commands::privacy::privacy_get_crash_reporting", effect: Effect::Read },
    PolicyEntry { path: "commands::privacy::privacy_set_crash_reporting", effect: Effect::Reversible },

    // commands/support.rs
    // `dest` is a caller-supplied path passed straight to `std::fs::File::create`,
    // which TRUNCATES an existing file at that path — an arbitrary pre-existing
    // file there is unrecoverably overwritten with the diagnostics bundle. No
    // record to prove reading (the caller already supplies `dest` themselves);
    // the running app version is the strongest available unrelated signal —
    // it IS embedded in the diagnostics bundle itself, but proves nothing
    // about `dest`; WEAK, flagged.
    PolicyEntry {
        path: "commands::support::support_export_diagnostics",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        }),
    },
    // Zero renderer references (ADR-038 Context). ADR-038 §2 revision (same
    // reasoning as `ai_unload_model` above): the current body is a literal
    // stub (`// Stub - implement when needed`) that always returns `null` —
    // Read would let `agent call` dispatch it and hand back `null` as if it
    // were genuine system info. NotExposed until the body is real.
    PolicyEntry {
        path: "commands::support::support_get_system_info",
        effect: Effect::NotExposed(
            "stub — the body is unimplemented and always returns null; dispatching it by name \
             would hand back null as if it were genuine system info",
        ),
    },

    // commands/dialog.rs
    PolicyEntry {
        path: "commands::dialog::dialog_open_files",
        effect: Effect::NotExposed(
            "blocks on a native OS multi-file-picker dialog (tauri_plugin_dialog::blocking_pick_files); \
             no argv/JSON equivalent for a non-interactive caller",
        ),
    },

    // commands/geocoding.rs
    PolicyEntry { path: "commands::geocoding::geocode_suggest", effect: Effect::Read },

    // commands/autopilot.rs
    PolicyEntry { path: "commands::autopilot::autopilot_list", effect: Effect::Read },
    PolicyEntry { path: "commands::autopilot::autopilot_get", effect: Effect::Read },
    PolicyEntry { path: "commands::autopilot::autopilot_create", effect: Effect::Reversible },
    PolicyEntry { path: "commands::autopilot::autopilot_update", effect: Effect::Reversible },
    // Deletes an autopilot record (and orphans its résumé-derived cache
    // rows). Proof is the target automation's own `name`, read via
    // `autopilot_get` by the SAME `autopilotId`.
    PolicyEntry {
        path: "commands::autopilot::autopilot_remove",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller("autopilotId"),
            path: &["name"],
        }),
    },
    // "Autopilot is a discovery agent... a run only finds, ranks and saves
    // results" (verified — no application submission), BUT the semantic
    // re-rank phase charges `charge_provider_daily` per embed when enabled
    // (`autopilot/rerank.rs::charge_one_embed`) — real, if capped, spend.
    // Same id-scoped proof as `autopilot_remove`.
    PolicyEntry {
        path: "commands::autopilot::autopilot_run",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller("autopilotId"),
            path: &["name"],
        }),
    },
    PolicyEntry { path: "commands::autopilot::autopilot_pause", effect: Effect::Reversible },
    PolicyEntry { path: "commands::autopilot::autopilot_resume", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::autopilot::autopilot_take_pending_focus",
        effect: Effect::NotExposed(
            "atomically consumes a buffered native window-focus intent meant for the renderer's \
             own window — same reasoning as menu_take_pending",
        ),
    },
    PolicyEntry { path: "commands::autopilot::autopilot_best_matches", effect: Effect::Read },

    // commands/ai_generations.rs
    PolicyEntry { path: "commands::ai_generations::ai_generations_list", effect: Effect::Read },
    PolicyEntry { path: "commands::ai_generations::ai_generations_save", effect: Effect::Reversible },
    PolicyEntry { path: "commands::ai_generations::ai_generations_update", effect: Effect::Reversible },
    // Deletes a generation AND cascades to delete its pipeline run trail
    // (`purge_run_trails`). Proof is the target generation's own `jobTitle`,
    // read via `ai_generations_list`, matched by id.
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "ai_generations_list",
            id_field: "id",
            match_field: "id",
            value_field: "jobTitle",
        }),
    },
    // Bulk delete by a caller-supplied `ids` array — no single record to
    // name. Proof is the COUNT of those ids that actually exist right now,
    // read via `ai_generations_list` — genuinely computed and scoped to the
    // targeted set, but a count, not a name; weaker than the single-id row
    // above, flagged.
    PolicyEntry {
        path: "commands::ai_generations::ai_generations_remove_bulk",
        effect: Effect::Irreversible(ProofSource::MatchCount {
            read_command: "ai_generations_list",
            ids_field: "ids",
            match_field: "id",
        }),
    },

    // commands/applications.rs
    PolicyEntry { path: "commands::applications::applications_list", effect: Effect::Read },
    PolicyEntry { path: "commands::applications::applications_get", effect: Effect::Read },
    PolicyEntry { path: "commands::applications::applications_set_status", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::applications::applications_accept_status_event",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::applications::applications_reject_status_event",
        effect: Effect::Reversible,
    },
    PolicyEntry { path: "commands::applications::applications_update", effect: Effect::Reversible },
    // Deletes an Application and (unless keep_documents) cascades to its
    // child generations. Proof is the target application's own `title`,
    // read via `applications_get` by the SAME `id`.
    PolicyEntry {
        path: "commands::applications::applications_delete",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "applications_get",
            key: "id",
            input: LookupInput::FromCaller("id"),
            path: &["application", "title"],
        }),
    },
    PolicyEntry { path: "commands::applications::applications_track", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::applications::applications_save_from_posting",
        effect: Effect::Reversible,
    },

    // commands/notifications.rs
    PolicyEntry { path: "commands::notifications::notifications_list", effect: Effect::Read },
    PolicyEntry { path: "commands::notifications::notifications_mark_read", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::notifications::notifications_mark_all_read",
        effect: Effect::Reversible,
    },
    // Proof is the target notification's own `title`, read via
    // `notifications_list`, matched by id.
    PolicyEntry {
        path: "commands::notifications::notifications_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "notifications_list",
            id_field: "id",
            match_field: "id",
            value_field: "title",
        }),
    },
    // Unconditional wipe of every notification — no selector. Proof is the
    // exact count about to be lost, read via `notifications_list` itself —
    // precisely scoped, the strongest shape a global wipe can have.
    PolicyEntry {
        path: "commands::notifications::notifications_clear_all",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "notifications_list",
        }),
    },
    PolicyEntry {
        path: "commands::notifications::notifications_clicked",
        effect: Effect::NotExposed(
            "focuses the desktop window and emits an event to open the inbox; meaningless for a \
             non-interactive caller with no window to focus",
        ),
    },

    // commands/referrals.rs
    PolicyEntry { path: "commands::referrals::referrals_list", effect: Effect::Read },
    PolicyEntry { path: "commands::referrals::referrals_upsert", effect: Effect::Reversible },
    // Proof is the target referral's own `companyName`, read via
    // `referrals_list`, matched by id.
    PolicyEntry {
        path: "commands::referrals::referrals_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "referrals_list",
            id_field: "id",
            match_field: "id",
            value_field: "companyName",
        }),
    },

    // commands/profile_import.rs
    PolicyEntry { path: "commands::profile_import::profile_import_from_url", effect: Effect::Read },

    // commands/github.rs
    PolicyEntry { path: "commands::github::github_import_repos", effect: Effect::Read },

    // commands/extension_bridge.rs
    PolicyEntry { path: "commands::extension_bridge::extension_bridge_status", effect: Effect::Read },
    // Rotates the pairing token, which REVOKES every currently-paired
    // browser session — the same "sign-out"-shaped irreversibility ADR-038
    // names for `sign_out_all`. No record exists besides the token itself,
    // and the CLI already possesses today's token locally (it needed it to
    // authenticate this very connection) — reading it back through
    // `extension_bridge_status` proves nothing the caller didn't already
    // have; the WEAKEST kind of proof in this table, flagged.
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_regenerate_token",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "extension_bridge_status",
            path: &["token"],
        }),
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_autofill_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_autofill_enabled",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_ai_assist_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_ai_assist_enabled",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_auto_track_enabled",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_set_auto_track_enabled",
        effect: Effect::Reversible,
    },

    // commands/email_watch.rs
    PolicyEntry { path: "commands::email_watch::email_watch_status", effect: Effect::Read },
    // Writes an IMAP app-password secret into the OS keychain —
    // `credentials:*`. A fresh connect has nothing to compare against; the
    // strongest available signal is whether an account is ALREADY connected
    // — WEAK (boolean, mostly `false` on the common first-connect path),
    // flagged.
    PolicyEntry {
        path: "commands::email_watch::email_watch_connect",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "email_watch_status",
            path: &["connected"],
        }),
    },
    // Removes the stored secret; also clears the auto-write opt-in + every
    // seen-mail dedupe row (verified — `EmailWatchStore::clear`'s own doc).
    // Proof is the CONNECTED ACCOUNT'S own address, read via
    // `email_watch_status` — real, user-owned data; genuinely requires
    // having read it first.
    PolicyEntry {
        path: "commands::email_watch::email_watch_disconnect",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "email_watch_status",
            path: &["address"],
        }),
    },
    PolicyEntry { path: "commands::email_watch::email_watch_set_enabled", effect: Effect::Reversible },
    PolicyEntry {
        path: "commands::email_watch::email_watch_set_auto_write_enabled",
        effect: Effect::Reversible,
    },
    // Rate-limited (60s) fetch+parse+match+notify pass; may flip a tracked
    // Application's status when auto-write is on — reversible via
    // applications_set_status, nothing destroyed.
    PolicyEntry { path: "commands::email_watch::email_watch_check_now", effect: Effect::Reversible },

    // export/commands/mod.rs
    // Verified: renders and returns bytes only — no filesystem write.
    PolicyEntry { path: "export::commands::documents_export_document", effect: Effect::Read },
    PolicyEntry {
        path: "export::commands::documents_export_and_save",
        effect: Effect::NotExposed(
            "renders the document then blocks on a native OS save-file dialog \
             (tauri_plugin_dialog::blocking_save_file); no argv/JSON equivalent for a \
             non-interactive caller",
        ),
    },
    PolicyEntry { path: "export::commands::documents_render_preview_images", effect: Effect::Read },

    // updater/mod.rs
    // Mutates in-memory `UpdaterState` only (pending version/bytes) — no persisted write.
    PolicyEntry { path: "updater::updater_check", effect: Effect::Reversible },
    // Downloads the update artifact into memory/state — not yet applied, nothing destroyed.
    PolicyEntry { path: "updater::updater_download", effect: Effect::Reversible },
    // Installs the downloaded update and force-restarts the app
    // (`app.restart()`, never returns) — replaces the running binary with
    // no undo path. No Read row exposes the PENDING (target) version —
    // `UpdaterState.pending_version` lives only in memory behind
    // `updater_check`/`updater_download`, both `Reversible` not `Read`, so
    // neither is eligible as a proof source. `system_get_version` is the
    // strongest available Read row, but it names the CURRENTLY RUNNING
    // version, not the one about to be installed — one of the WEAKEST rows
    // in this table, flagged prominently alongside `privacy_reset_app`.
    PolicyEntry {
        path: "updater::updater_install",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        }),
    },
    PolicyEntry { path: "updater::updater_changelog", effect: Effect::Read },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The `lib.rs` source, embedded at compile time — the SAME text
    /// `cargo build` feeds to `tauri::generate_handler!`, so extraction from
    /// it can never drift from what is actually wired up (mirrors
    /// `commands::cli_agents::tests`' `include_str!` of the capability
    /// allowlist for the identical reason).
    const LIB_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));

    /// Extract the fully-qualified command paths registered inside
    /// `tauri::generate_handler![...]`, in source order. Comment-only and
    /// blank lines inside the list are skipped; every other line is
    /// trimmed of its trailing comma. Panics (test-only) if the markers
    /// this depends on ever move — that failure itself is the signal this
    /// extraction needs updating, not a silent empty result.
    ///
    /// The closing `]` is located on the first NON-comment line that
    /// carries one (LOW fix — security review): the naive `rest.find(']')`
    /// this used to run against the RAW text would truncate the extraction
    /// early if a `//` comment between the marker and the real terminator
    /// ever contained a literal `]` — silent, since a truncated-but-still-
    /// well-formed list still passes both anti-drift tests below with a
    /// SMALLER `POLICY` and a smaller `registered` set, never surfacing the
    /// mismatch. Comment lines are skipped when searching for `]`, not when
    /// slicing — every real command line up to the true terminator is kept.
    fn registered_command_paths() -> Vec<&'static str> {
        const START_MARKER: &str = "tauri::generate_handler![";
        let start = LIB_RS
            .find(START_MARKER)
            .expect("tauri::generate_handler![ marker present in lib.rs")
            + START_MARKER.len();
        let rest: &'static str = &LIB_RS[start..];

        let mut end = None;
        let mut offset = 0usize;
        for line in rest.lines() {
            let is_comment = line.trim_start().starts_with("//");
            if !is_comment {
                if let Some(pos) = line.find(']') {
                    end = Some(offset + pos);
                    break;
                }
            }
            // `lines()` strips the `\n` each line ended with — add it back
            // so `offset` stays a correct byte position into `rest`.
            offset += line.len() + 1;
        }
        let end =
            end.expect("generate_handler! list has a closing ] on a non-comment line in lib.rs");

        rest[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(|line| line.trim_end_matches(','))
            .collect()
    }

    /// Hand-written literal — deliberately NOT derived from `POLICY.len()`
    /// or from `registered_command_paths()`. A test that only loops over
    /// the table it guards covers additions only (this repo's own standing
    /// lesson: `feedback_a_guard_driven_off_its_own_data_cannot_catch_a_deletion`)
    /// — this fails independently of either source's own content.
    #[test]
    fn policy_table_has_exactly_164_rows() {
        assert_eq!(POLICY.len(), 164);
    }

    /// ADR-038 §1's core invariant: the policy table and `generate_handler!`
    /// agree EXACTLY — no command reachable from the registry without a
    /// classified row, and no stale/typo'd row claiming a command that
    /// isn't actually registered. Both directions, with the offending
    /// command named in the failure message.
    #[test]
    fn policy_table_matches_generate_handler_exactly() {
        let registered: HashSet<&str> = registered_command_paths().into_iter().collect();
        let policy: HashSet<&str> = POLICY.iter().map(|e| e.path).collect();

        let missing: Vec<&&str> = registered.difference(&policy).collect();
        assert!(
            missing.is_empty(),
            "commands registered in generate_handler! but missing a POLICY row \
             (unclassified — must be added): {missing:?}"
        );

        let extra: Vec<&&str> = policy.difference(&registered).collect();
        assert!(
            extra.is_empty(),
            "POLICY rows with no matching generate_handler! registration \
             (stale, or the path is typo'd): {extra:?}"
        );
    }

    /// Guards the set-equality test above against a duplicate masking a
    /// missing row: two identical `path`s would satisfy both `difference`
    /// checks while a third, genuinely-unclassified command silently has
    /// no row at all.
    #[test]
    fn policy_table_has_no_duplicate_paths() {
        let mut seen = HashSet::new();
        for entry in POLICY {
            assert!(
                seen.insert(entry.path),
                "duplicate POLICY row for {}",
                entry.path
            );
        }
    }

    /// Every `NotExposed` row carries a real, specific reason — never a
    /// bare placeholder like "unclear" or "todo" (rule 3 of the
    /// classification pass this table was built under).
    #[test]
    fn not_exposed_rows_carry_a_real_reason() {
        for entry in POLICY {
            if let Effect::NotExposed(reason) = entry.effect {
                assert!(
                    reason.trim().len() > 15,
                    "{} is NotExposed with a too-short/placeholder reason: {reason:?}",
                    entry.path
                );
            }
        }
    }

    /// `registered_command_paths` itself must find every command `lib.rs`
    /// actually registers — sanity-checks the extraction against a handful
    /// of paths spanning the start, middle and end of the list, so a
    /// regression in the marker/parsing logic (not just a POLICY drift)
    /// is caught here rather than surfacing as a confusing mismatch above.
    #[test]
    fn extraction_finds_known_paths_at_each_end_of_the_list() {
        let found = registered_command_paths();
        assert_eq!(
            found.first().copied(),
            Some("commands::cli_agents::cli_agents_status"),
            "extraction must find the FIRST registered command"
        );
        assert_eq!(
            found.last().copied(),
            Some("updater::updater_changelog"),
            "extraction must find the LAST registered command"
        );
        assert!(found.contains(&"commands::privacy::privacy_reset_app"));
        assert_eq!(found.len(), 164);
    }

    /// ADR-038 §4 (Phase 3): every `Irreversible` row's
    /// `ProofSource::read_command` must itself be a REAL `Effect::Read` row
    /// in this SAME table — the ceremony's whole safety property rests on
    /// the proof coming from a surface this table has independently
    /// classified as safe to dispatch freely. A `ProofSource` pointing at a
    /// command that doesn't exist, or exists but isn't `Read`, would make
    /// the ceremony either uncheckable or a second mutation smuggled in
    /// under "reading the proof".
    #[test]
    fn every_proof_source_read_command_is_a_read_row() {
        let mut checked = 0usize;
        for entry in POLICY {
            let Effect::Irreversible(source) = entry.effect else {
                continue;
            };
            checked += 1;
            let read_command = source.read_command();
            let target = POLICY
                .iter()
                .find(|e| e.path.rsplit("::").next() == Some(read_command));
            match target {
                Some(t) if t.effect == Effect::Read => {}
                Some(t) => panic!(
                    "{}'s ProofSource points at `{read_command}`, which is classified \
                     {t:?}, not Read",
                    entry.path
                ),
                None => panic!(
                    "{}'s ProofSource points at `{read_command}`, which has no POLICY row \
                     at all",
                    entry.path
                ),
            }
        }
        // Hand-written literal (not derived from POLICY itself — the same
        // "pair a loop with a literal" discipline as
        // `policy_table_has_exactly_164_rows`): 31 Irreversible rows.
        assert_eq!(checked, 31, "expected exactly 31 Irreversible rows");
    }

    /// Mutation-style guard: an `Irreversible` row whose `ProofSource`
    /// pointed at ITSELF (or at another `Irreversible` row) would make the
    /// ceremony circular — satisfiable without ever reading anything real.
    #[test]
    fn no_proof_source_points_at_an_irreversible_command() {
        for entry in POLICY {
            if let Effect::Irreversible(source) = entry.effect {
                let (_, own_command) = entry.path.rsplit_once("::").unwrap_or(("", entry.path));
                assert_ne!(
                    source.read_command(),
                    own_command,
                    "{} names itself as its own proof source",
                    entry.path
                );
            }
        }
    }
}
