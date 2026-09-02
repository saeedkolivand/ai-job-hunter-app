//! ADR-038 §1 — the vocabulary [`super::POLICY`] is written in: the
//! consequence class of one command ([`Effect`]), where the confirmation
//! ceremony reads its `--confirm` value from ([`ProofSource`] /
//! [`LookupInput`]), and one table row ([`PolicyEntry`]).
//!
//! Split out of `policy.rs` under R8's hard LOC cap — the same reason
//! `policy/tests.rs` and `documents/embedding.rs` exist. The division is not
//! arbitrary: this file holds the types and the RULES a row is classified
//! by, and NOT ONE ROW, so `policy.rs` is now the table and nothing else.
//! Everything here is re-exported at `super::` (see the `pub(crate) use
//! types::…` line there), so the split is invisible to every existing
//! `policy::Effect` / `policy::ProofSource` call site.
//!
//! **Read this module's doc before adding or reclassifying a row.** The
//! classification rules below are the whole reason a row's `Effect` is a
//! declared fact rather than a guess from its name.
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
//! `every_proof_source_read_command_is_a_read_row` in
//! `super::tests`), so possessing it
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

//! ## Round 3 addendum — a destination is an effect too
//! Every row in [`super::POLICY`] was classified on ONE axis: does the command
//! persist anything. A caller-supplied `url`/`host`/`path`/`base_url` argument
//! SECOND axis this table under-weighted: `ai_test_provider_key`/
//! `ai_list_provider_models` (an egress host for a keychain secret) and
//! `resume::extract_resume` (a filesystem path) were all `Read` — nothing
//! persisted, so nothing on the first axis flagged them — while the
//! caller's own input chose where a secret or a file read landed. Fixed
//! here (their own row comments carry the detail); `scrape_url`/
//! `scrape_resolve_url`/`profile_import_from_url`/`github_import_repos` also
//! take a caller-controlled destination but were individually verified
//! (their own row comments) to carry no secret and no unbounded host choice,
//! so they are unchanged.

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
/// registers (matched verbatim against `lib.rs` by `super`'s own
/// exactness test), plus its declared [`Effect`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct PolicyEntry {
    /// The fully-qualified path exactly as it appears inside
    /// `tauri::generate_handler![...]` in `lib.rs`, e.g.
    /// `"commands::jobs::jobs_list"`.
    pub(crate) path: &'static str,
    pub(crate) effect: Effect,
}
