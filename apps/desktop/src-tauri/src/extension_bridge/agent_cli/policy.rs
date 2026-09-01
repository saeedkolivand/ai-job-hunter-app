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
//!   mutated no state (a legitimate `Read` call on that axis alone) but hit
//!   a paid embedding provider with no `charge_provider_daily`/
//!   `limiter.acquire` gate anywhere in its call chain, so dispatching it by
//!   name would have let a caller spend against a paid provider with zero
//!   budget enforcement — `NotExposed` until that gate landed
//!   (`commands::ai::ai_embed`'s own `admit_embed`). Once it did, the SAME
//!   spend-against-a-paid-provider's-per-day-ceiling rule that makes
//!   `ai_generate` [`Effect::Irreversible`] (not `Read`) applies here too —
//!   see `ai_embed`'s own row comment for why a `charge_provider_daily`
//!   gate is what triggers Irreversible, not what exempts a row from it.
//!   `match_resume`/`match_resume_text` carry the CURRENT live example of
//!   this same no-gate case (their row comments).
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
//!   made that classification reachable, not merely descriptive); a real
//!   read/write with no anti-abuse gate on its own paid egress
//!   (`match_resume`/`match_resume_text`, reclassified from `Reversible`
//!   once Phase 4 made the SAME thing true of `ai_embed`'s blast radius
//!   before ITS gate landed — see those two rows' own comments); or an
//!   `Irreversible` command whose ONLY reachable [`ProofSource`] is
//!   provably vacuous — not merely weak, but a value the caller is
//!   structurally guaranteed to already hold or that reads as a constant for
//!   the whole duration of the ceremony (`extension_bridge_regenerate_token`,
//!   reclassified once Phase 3 made a proof requirement reachable for it —
//!   see that row's own comment for the two independent reasons neither of
//!   `extension_bridge_status`'s fields can ever bind this ceremony to
//!   anything the caller didn't already know).
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
//!   to be lost, scaled to the actual blast radius wherever a Read row
//!   reaches the PRIMARY store being wiped rather than a secondary one
//!   (`notifications_clear_all`, `privacy_clear_interactions`,
//!   `privacy_reset_app`), or, honestly, a WEAK fallback with no real
//!   binding to the specific target (`system_open_external`,
//!   `updater_install`) — flagged per-row rather than dressed up as strict.
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
    /// `value_field` off the element whose `match_field` equals the value at
    /// `id_field` — a PATH walked into the irreversible command's own
    /// `--input` body (same shape/reasoning as `LookupInput::FromCaller`
    /// above) — the "delete by id, prove you read its name" shape.
    ListMatch {
        read_command: &'static str,
        id_field: &'static [&'static str],
        match_field: &'static str,
        value_field: &'static str,
    },
    /// `read_command` takes no input and returns an array; the proof is its
    /// length — the strongest available signal for a selector-less wipe
    /// (the module doc's "no record exists" case).
    Count { read_command: &'static str },
    /// `read_command` takes no input and returns an array; the proof is the
    /// count of its elements whose `match_field` is a member of the value at
    /// `ids_field` — a PATH walked into the irreversible command's own
    /// `--input` body (a JSON array) — `ai_generations_remove_bulk`'s own
    /// bulk-selector shape.
    MatchCount {
        read_command: &'static str,
        ids_field: &'static [&'static str],
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
    /// Walk this PATH into the irreversible command's own `--input` body
    /// (via the same `walk()` `agent_call::proof` already uses for a
    /// response path), then forward the value as
    /// `read_command`'s SAME-named input field — the caller already
    /// supplied it to target this exact record. A single-element path
    /// (`&["id"]`) is a flat top-level field; a longer one (`&["req",
    /// "runId"]`) reaches into a command whose own `#[tauri::command]`
    /// signature takes one wrapped `req: SomeRequest` argument (HIGH fix —
    /// security review: a flat single-field selector silently read the
    /// WRONG location — the ceremony's own top level — for exactly this
    /// shape, making the row either permanently unsatisfiable or, worse,
    /// satisfiable against a record the real command never acts on; see
    /// `resume_pipeline_regenerate_section`'s row comment for the concrete
    /// exploit).
    FromCaller(&'static [&'static str]),
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
    // ADR-038 §4 revision, reclassified AGAIN (HIGH fix — security review
    // round 2): the prior revision here reclassified this row `NotExposed`
    // because `documents::embed` → `embed_text` hit a PAID embedding
    // provider with NO `charge_provider_daily`/`limiter.acquire` gate at
    // all. That gate landed (`admit_embed`, this file's own body — see
    // `ai_embed`'s doc comment there), and its own STATUS note said this row
    // would then flip to `Read`. It does NOT: `charge_provider_daily`
    // spending against a paid provider's per-day ceiling is this table's own
    // named Irreversible trigger (module doc, "ADR-038 itself names four
    // canonical patterns" paragraph) — applied to every OTHER command that
    // charges it (`ai_generate`, `ai_research_company`, `ai_reembed_all`,
    // and `autopilot_run`'s own per-embed charge via `RerankBudget`, all
    // Irreversible below). Flipping this ONE row to `Read` instead would
    // make it the sole freely-dispatchable paid-provider-spend command in
    // the table — the exact inconsistency Finding 5 (this same review round)
    // flags for `match_resume`/`match_resume_text`. Same WEAK spend-total
    // proof as every other ungapped AI-spend row: no id-scoped record exists
    // to prove against (the request is a bare text blob), so the caller's
    // own today-so-far spend is the strongest available signal.
    PolicyEntry {
        path: "commands::ai::ai_embed",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        }),
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
            input: LookupInput::FromCaller(&["provider"]),
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
            input: LookupInput::FromCaller(&["provider"]),
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
    // `id_field: &["req", "resumeId"]` (HIGH fix — security review round 2):
    // `resume_pipeline_run`'s own `#[tauri::command]` signature takes ONE
    // wrapped `req: ResumePipelineRunRequest` argument, so the wire body is
    // `{"req": {"resumeId": ..., ...}}` — a flat `"resumeId"` selector read
    // the ceremony's own TOP LEVEL, where the caller's real `resumeId` never
    // lives, making this row permanently unsatisfiable (`proof_unavailable`
    // on every attempt). `match_field: "_id"` (also HIGH fix): `documents_
    // list` returns `DocumentRecord`, which serializes its id as `_id`
    // (`#[serde(rename = "_id")]`), not `id` — the SAME field this repo has
    // already been bitten by once (`reference_documentrecord_id_is_not_the_
    // wire_shape`), walked into again here.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_run",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["req", "resumeId"],
            match_field: "_id",
            value_field: "name",
        }),
    },
    PolicyEntry { path: "commands::resume_pipeline::resume_pipeline_get", effect: Effect::Read },
    PolicyEntry { path: "commands::resume_pipeline::resume_pipeline_list_for_job", effect: Effect::Read },
    // Same charged AI-regenerate path as `resume_pipeline_run`, scoped to a
    // real run (`runId`) — proof is that run's own `jobUrl`, read via
    // `resume_pipeline_get`. `LookupInput::FromCaller(&["req", "runId"])`
    // (HIGH fix — security review round 2, worse than the row above):
    // `resume_pipeline_regenerate_section` ALSO takes one wrapped `req`
    // argument, so a flat `"runId"` selector didn't just fail to resolve —
    // Tauri silently ignores unknown TOP-LEVEL body keys, so
    // `--input '{"runId":"A","req":{"runId":"B",...}}'` resolved the proof
    // against run A's `jobUrl` while the real command acted on run B: an
    // UNBOUND ceremony, satisfiable by reading a record the command never
    // touches. Reading from `req.runId` — the SAME location the command
    // itself reads its target from — closes that; see
    // `the_real_resume_pipeline_regenerate_section_policy_row_ignores_a_
    // decoy_top_level_run_id` (proof tests, run against THIS row) and
    // `build_input_ignores_a_decoy_top_level_field_and_reads_only_the_
    // wrapped_path` (the hand-typed pure-logic pin) for the regression
    // guards.
    PolicyEntry {
        path: "commands::resume_pipeline::resume_pipeline_regenerate_section",
        effect: Effect::Irreversible(ProofSource::Lookup {
            read_command: "resume_pipeline_get",
            key: "runId",
            input: LookupInput::FromCaller(&["req", "runId"]),
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
    // `match_field: "_id"` (HIGH fix — security review round 2):
    // `documents_list` returns `DocumentRecord`, whose id serializes as
    // `_id` (`#[serde(rename = "_id")]`), not `id` — see
    // `resume_pipeline_run`'s row comment above for the shared root cause.
    // `id_field: "id"` is unaffected: `documents_remove`'s own signature
    // takes a flat top-level `id`, not a wrapped `req`.
    PolicyEntry {
        path: "commands::documents::documents_remove",
        effect: Effect::Irreversible(ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "_id",
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
    // ADR-038 §4 revision (HIGH fix — security review round 2), reclassified
    // from `Reversible`: mutating a recomputable match-score cache row is
    // genuinely `Reversible` on ITS OWN axis, but when
    // `semanticScoringEnabled: true` is passed and the (resume, job) pair
    // isn't already cached, `score_one` reaches a PAID embedding provider
    // (`embed_charged` → `documents::embed`) with `budget: None` — verified
    // at both call sites (`match_resume`/`score_resume_against_text`'s own
    // doc comment: "user-initiated: not charged against the unattended
    // daily ceiling"). No `charge_provider_daily` gate exists on this path
    // at all, so `agent call` dispatching it by name (looping with a fresh
    // `jobText`/`jobId` each call to defeat the content-addressed cache) is
    // the SAME uncapped-paid-provider-spend property that forced `ai_embed`
    // NotExposed until IT was gated — see `ai_embed`'s row and the module
    // doc's "How each row was classified" section. NotExposed until a real
    // charge is threaded through (a change to the interactive scoring path
    // shared with the renderer, out of scope for this table).
    PolicyEntry {
        path: "commands::match_resume::match_resume",
        effect: Effect::NotExposed(
            "reaches a paid embedding provider (score_one → embed_charged) whenever \
             semanticScoringEnabled=true and the (resume,job) pair is not already cached, but \
             passes budget=None — no charge_provider_daily gate exists on this path, so \
             dispatching it by name would let a caller spend against a paid embedding provider \
             with zero daily-budget cap by varying the job each call; the gap is pre-existing \
             and shared with the interactive UI, so a real charge is a separate change, not \
             this table's fix",
        ),
    },
    PolicyEntry {
        path: "commands::match_resume::match_resume_text",
        effect: Effect::NotExposed(
            "reaches a paid embedding provider (score_one → embed_charged) whenever \
             semanticScoringEnabled=true and the (resume,job) pair is not already cached, but \
             passes budget=None — no charge_provider_daily gate exists on this path, so \
             dispatching it by name would let a caller spend against a paid embedding provider \
             with zero daily-budget cap by varying the job text each call; the gap is \
             pre-existing and shared with the interactive UI, so a real charge is a separate \
             change, not this table's fix",
        ),
    },
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
    // full factory reset. No Read row captures the full blast radius (19
    // stores registered via `manage_resettable`, `data_store.rs`); the proof
    // now reads `applications_list` rather than `ai_generations_list` —
    // reclassified (security review on this PR): `ApplicationStore` IS one
    // of the 19 (`reg.register::<ApplicationStore>("applications")`,
    // `commands/privacy.rs`'s own reset-registry test), and it is the
    // PRIMARY user-authored record this app exists to hold (the tracked job
    // search itself), not a secondary/derived table of AI-generated text —
    // `ai_generations_list` counted a DIFFERENT store's rows and proved
    // nothing about the applications actually at risk. This also scales
    // correctly with the thing ADR-038 §4 cares about: a user with 200
    // tracked applications is protected by a genuinely unguessable number,
    // and a user with 0 has nothing of substance in the store this counts —
    // still PARTIAL (one of 19 stores, not the full blast radius) and still
    // flagged, but bound to the core data rather than a side table.
    PolicyEntry {
        path: "commands::privacy::privacy_reset_app",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "applications_list",
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
            input: LookupInput::FromCaller(&["autopilotId"]),
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
            input: LookupInput::FromCaller(&["autopilotId"]),
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
            id_field: &["id"],
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
            ids_field: &["ids"],
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
            input: LookupInput::FromCaller(&["id"]),
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
            id_field: &["id"],
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
            id_field: &["id"],
            match_field: "id",
            value_field: "companyName",
        }),
    },

    // commands/profile_import.rs
    // Network (fetches the given profile url), but no PAID provider in the
    // chain — `Read` on both axes (no persisted-state change, no un-metered
    // billable spend), unlike `ai_embed` above. VERIFIED (recorded so this
    // doesn't get re-raised every review pass): takes a CALLER-SUPPLIED url
    // as its own argument, so `agent call` makes this an egress primitive
    // reachable from the CLI — the SAME reach the UI already has (a user
    // pastes a profile url there too), not a new one this table opens.
    // `import_from_url`'s own `detect_platform` host-allowlists the fetch to
    // linkedin.com (exact/suffix match on the URL's HOST, never a substring
    // scan — guards the `attacker.example/linkedin.com/...` lookalike and a
    // loopback-egress url), so the caller controls WHICH linkedin.com path
    // is fetched, never an arbitrary destination host.
    PolicyEntry { path: "commands::profile_import::profile_import_from_url", effect: Effect::Read },

    // commands/github.rs
    // Network, no paid provider in the chain — same `Read` reasoning as
    // `profile_import_from_url` above. VERIFIED, and UNLIKE that row: this
    // DOES take a caller-supplied `input` (a username or github.com url),
    // but `parse_username` extracts+validates it into a bare username (SSRF
    // guard — rejects a metadata-service url, e.g. `169.254.169.254`, and
    // anything not github.com-shaped) before `api_url` builds a request
    // against the FIXED host `api.github.com` — the caller controls a path
    // segment, never the destination host, so this is not an arbitrary-url
    // egress primitive the way `profile_import_from_url` is.
    PolicyEntry { path: "commands::github::github_import_repos", effect: Effect::Read },

    // commands/extension_bridge.rs
    PolicyEntry { path: "commands::extension_bridge::extension_bridge_status", effect: Effect::Read },
    // Rotates the pairing token, which REVOKES every currently-paired
    // browser session. Reclassified NotExposed (was `Irreversible` with a
    // `ProofSource::Scalar` over `extension_bridge_status`) — VERIFIED that
    // NO proof reachable from this table can ever bind this ceremony to
    // anything the caller didn't already have, for two INDEPENDENT reasons,
    // not one:
    //   1. `port` and `token` are values this exact CLI connection already
    //      needed to exist at all — it scanned `PORT_RANGE` to find the port
    //      and read the plaintext token off disk to compute the handshake's
    //      `client_proof` (`agent_cli.rs::read_pairing_token`/
    //      `connect_authenticated`), so echoing either back through
    //      `extension_bridge_status` proves nothing.
    //   2. `connected` is worse than merely a low-entropy boolean: it is
    //      `BridgeState.connected > 0`, and this VERY call's own socket
    //      already incremented that same counter on authenticating
    //      (`inc_connected`, `extension_bridge/mod.rs`) — `agent.call` only
    //      reaches here over an already-`Authenticated` connection, so
    //      `connected` reads `true` for the ENTIRE duration of every possible
    //      ceremony attempt, regardless of whether any browser is paired. A
    //      hallucinated `true` does not merely have decent odds; it is
    //      GUARANTEED to match.
    // No other Read row in the `extension_bridge` namespace is bound to the
    // pairing session this revokes either — `extension_bridge_autofill_
    // enabled`/`ai_assist_enabled`/`auto_track_enabled` are unrelated
    // feature toggles, no stronger than the rejected `connected` boolean.
    // Falling back to a totally unrelated Read row (the `system_get_version`
    // pattern `system_open_external`/`updater_install` use when no domain
    // read exists at all) would be decorative here, not merely weak: unlike
    // those two — which are ADR-038-adjacent but not one of its four NAMED
    // canonical Irreversible patterns (`privacy:reset_app`, `sign_out_all`,
    // `credentials:*`, `*_remove`) — this command's only real-world effect
    // is breaking the user's OWN browser-extension pairing (a UI Settings
    // "Regenerate" self-service action a human performs while looking at the
    // resulting "re-pair your browser" state), with no job-hunting workflow
    // that plausibly motivates an autonomous agent driving it. A ceremony
    // that can never fail is worse than an honest refusal.
    PolicyEntry {
        path: "commands::extension_bridge::extension_bridge_regenerate_token",
        effect: Effect::NotExposed(
            "rotating the pairing token has no reachable non-vacuous proof: every field of \
             extension_bridge_status (port, token) is a value this exact connection already \
             had to possess to authenticate, and `connected` reads true for the whole call \
             because this CLI socket's own authentication is what increments it — see the \
             row's own comment for the full two-part argument",
        ),
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
    // in this table, flagged prominently.
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

    /// HIGH fix (security review round 2): `match_resume`/`match_resume_text`
    /// reach a paid embedding provider (`score_one` → `embed_charged`) with
    /// `budget: None` when `semanticScoringEnabled: true` — the SAME
    /// uncapped-spend shape that forced `ai_embed` `NotExposed` before ITS
    /// gate landed. Hand-written (not looped, mirroring
    /// `policy_table_has_exactly_164_rows`'s own discipline): a revert of
    /// either row back to `Reversible` (freely dispatchable, no confirm, no
    /// cap) would not be caught by any OTHER test in this file — the
    /// Irreversible-row count is untouched by a `Reversible` change, and
    /// `not_exposed_rows_carry_a_real_reason` only checks rows that ARE
    /// `NotExposed`, never that a specific row remains one.
    #[test]
    fn match_resume_and_match_resume_text_stay_not_exposed_until_a_real_charge_lands() {
        for path in [
            "commands::match_resume::match_resume",
            "commands::match_resume::match_resume_text",
        ] {
            let entry = POLICY
                .iter()
                .find(|e| e.path == path)
                .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
            assert!(
                matches!(entry.effect, Effect::NotExposed(_)),
                "{path} must stay NotExposed (uncharged paid-embedding path) until a real \
                 charge_provider_daily gate lands on it — got {:?}",
                entry.effect
            );
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
        // `policy_table_has_exactly_164_rows`): 31 Irreversible rows
        // (`extension_bridge_regenerate_token` moved to `NotExposed` —
        // security review round 1; `ai_embed` moved NotExposed → Irreversible
        // once its `charge_provider_daily` gate landed, and
        // `match_resume`/`match_resume_text` moved Reversible → NotExposed
        // for the SAME reason `ai_embed` originally was — security review
        // round 2, see each row's own comment).
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
