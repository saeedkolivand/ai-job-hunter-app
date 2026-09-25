//! Discovery, help, data, dedup, matching, credentials, boards,
//! privacy, support, dialogs and geocoding.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const DISCOVERY_AND_ACCOUNT: &[PolicyEntry] = &[
    // commands/hybrid_search.rs — embeds + rerank charge charge_provider_daily (`ai_embed`'s trigger).
    // Same no-id / WEAK spend-total fallback as `ai_generate`, and flagged one step further here:
    // `today.inputTokens` reads back as the string for zero on a day nothing has been spent yet,
    // and a caller can produce "0" without reading anything — so on such a day this ceremony is
    // vacuous in `system_open_external`'s own sense, not merely weak, and binds to nothing. Kept
    // Irreversible DELIBERATELY on that understanding: its job is friction against an accidental
    // or looping call, not authenticating which search is about to run.
    PolicyEntry {
        path: "commands::hybrid_search::scrape_hybrid_search",
        effect: Effect::Irreversible(ProofSource::Scalar { read_command: "ai_spend_summary", path: &["today", "inputTokens"] }),
    },
    // commands/help.rs — reclassified `Irreversible` → `NotExposed` (issue
    // #1169): the corpus stays in the renderer's own translation bundles
    // (ADR-043 — `commands::help::help_search`'s own module doc, "The
    // corpus stays in the translation bundles"), and `entries` is the
    // caller's OWN request field, not derived from anything Rust reads. No
    // command on this dispatcher can supply it — there is no
    // `help_entries`/`help_corpus` read anywhere in `POLICY` — so an
    // `agent call`/MCP caller has no way to reach this command with a real
    // corpus; dispatching it by name would only ever run a search over
    // whatever text the caller typed into `entries` itself.
    PolicyEntry {
        path: "commands::help::help_search",
        effect: Effect::NotExposed(
            "the help corpus lives only in the renderer's translation bundles (ADR-043) and \
             is sent as this command's own `entries` request field; no other command on this \
             dispatcher can supply it, so a CLI/MCP caller has no real corpus to search",
        ),
    },
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
    // HIGH fix (security review round 3), reclassified from `Irreversible`:
    // the module doc's own NotExposed rule (clause 2, "How each row was
    // classified" above) is "an Irreversible command whose ONLY reachable
    // ProofSource is provably vacuous — not merely weak, but a value the
    // caller is structurally guaranteed to already hold or that reads as a
    // constant for the whole duration of the ceremony." `system_get_version`
    // IS exactly that: `env!("CARGO_PKG_VERSION")`, a compile-time constant
    // for the whole process, published in the repo, the release feed and
    // the About tab — a caller never needs to call `system_get_version` at
    // all to know it. That was already true when this row was first
    // classified `Irreversible`/WEAK; the same vacuousness argument that
    // demoted `extension_bridge_regenerate_token` applies here and was
    // missed. `dest` is still a caller-supplied path passed straight to
    // `std::fs::File::create`, TRUNCATING an existing file there
    // unrecoverably with ZERO relationship between the confirmed value and
    // the actual target — the exact harm clause 2 exists to catch, not
    // merely a weak signal. `system_open_external`/`updater_install` ALSO
    // use `system_get_version` and are DELIBERATELY NOT reclassified here —
    // see their own rows for why the same vacuous-constant fact does not
    // carry the same harm for either of them.
    PolicyEntry {
        path: "commands::support::support_export_diagnostics",
        effect: Effect::NotExposed(
            "its only reachable ProofSource (system_get_version) is a compile-time constant, \
             published in the repo/release feed/About tab — the same vacuous-proof reasoning \
             that demoted extension_bridge_regenerate_token, and here it guards an unbounded \
             arbitrary-file-truncation via a caller-supplied dest with zero binding to it",
        ),
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

];
