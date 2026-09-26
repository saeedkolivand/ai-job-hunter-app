//! The read + generation surface: CLI agents, system, menu, jobs,
//! then every `commands/ai` generation and local-model command.
//!
//! One contiguous shard of `POLICY`, split out under R8's LOC cap and
//! concatenated back in `lib.rs`'s `generate_handler!` order by the parent.

use super::*;

pub(super) const CORE_AND_AI_GENERATION: &[PolicyEntry] = &[
    // commands/cli_agents.rs
    PolicyEntry {
        path: "commands::cli_agents::cli_agents_status",
        effect: Effect::Read,
    },
    // Clears + re-probes an in-process detection cache only (no persisted
    // write) — self-heals on the very next status call.
    PolicyEntry {
        path: "commands::cli_agents::cli_agents_redetect",
        effect: Effect::Reversible,
    },
    // commands/system/mod.rs
    PolicyEntry {
        path: "commands::system::system_health",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_get_version",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_get_locale",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_set_locale",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::system::system_get_platform",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_accent_color",
        effect: Effect::Read,
    },
    // Launches the OS's default http(s) handler (an external process this
    // app does not control) — scheme-allowlisted, but still an external
    // side effect with no undo, per the module doc's pattern list.
    // Correction (security review round 4): an earlier version of this
    // comment said "there is no caller-chosen target" — WRONG, and the
    // opposite of the module doc's own Round 3 addendum ("a destination is
    // an effect too"): the `url` argument IS a real caller-chosen target,
    // exactly that axis. What is actually true is narrower — there is no
    // SEPARATE record to check it against: any url the caller types is, by
    // definition, the one this ceremony would need to confirm, so there is
    // no "wrong url" shape a delete-by-id ceremony can have (targeting a
    // record other than the one the caller meant). The real safety boundary
    // is the scheme allowlist in the real command (http(s) only — no
    // `file://`, no custom scheme), which bounds HARM MAGNITUDE, not this
    // proof authenticating which destination is correct. Also a vacuous
    // compile-time constant by the same reasoning `support_export_
    // diagnostics` was reclassified for (security review round 3) — kept
    // Irreversible/WEAK here DELIBERATELY, not an oversight: its job is
    // friction against an accidental/looping call, not authenticating a
    // read of a specific record.
    PolicyEntry {
        path: "commands::system::system_open_external",
        effect: Effect::Irreversible(ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        }),
    },
    PolicyEntry {
        path: "commands::system::system_set_performance_mode",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::system::system_get_launch_at_login",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_set_launch_at_login",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::system::system_set_close_to_tray",
        effect: Effect::Reversible,
    },
    PolicyEntry {
        path: "commands::system::system_get_metrics",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_check_browser",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::system::system_open_devtools",
        effect: Effect::NotExposed(
            "opens a debugging devtools window on the app's own webview; meaningless for a \
             non-interactive caller with no window to look at, and nothing is returned",
        ),
    },
    PolicyEntry {
        path: "commands::system::system_get_protocol_version",
        effect: Effect::Read,
    },
    // Reclassified `NotExposed` from `Read` (external review of the MCP pass)
    // on the SAME recipient axis as `extension_bridge_status` below, applied
    // to a value that is not a secret. Nothing is persisted and nothing is
    // spent — the first axis this table was built on says `Read` — but the
    // response is an absolute filesystem path that on Windows and macOS sits
    // inside the user's home directory and therefore carries their account
    // name, and an MCP `call-read` client forwards every tool result to its
    // own cloud model provider and writes it into an on-disk transcript.
    //
    // The value exists for ONE recipient: the renderer's Settings → Developer
    // card, which renders ready-to-paste registration snippets. Handing it to
    // the agent tier adds nothing an agent can use — a client that reached
    // this dispatcher launched this binary and so already holds the path, and
    // a program that does not reads it from the launch-time pointer file
    // `~/.ajh-agent/agent.json`, which is the documented discovery contract
    // (`docs/knowledge/agent-cli.md`). So the exposure is all cost and no
    // capability.
    //
    // No `ProofSource` in this table reads this row (grep `read_command:
    // "system_agent_cli_info"`: zero hits), so no ceremony depends on it. The
    // `#[tauri::command]` stays registered and reachable from renderer IPC
    // exactly as before — only this CLI/MCP dispatch surface loses the ability
    // to name it.
    PolicyEntry {
        path: "commands::system::system_agent_cli_info",
        effect: Effect::NotExposed(
            "returns the absolute path of the app binary, which on Windows and macOS is a \
             home-directory path carrying the user's account name; a CLI/MCP caller launched \
             this binary and already holds that path (the pointer file ~/.ajh-agent/agent.json \
             is the discovery contract for one that did not), so exposing it here only puts a \
             user path into a persisted LLM transcript",
        ),
    },
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
    PolicyEntry {
        path: "commands::jobs::jobs_list",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::jobs::jobs_get",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::jobs::jobs_cancel",
        effect: Effect::Reversible,
    },
    // Verified: returns the job's kind/id for the RENDERER to re-dispatch —
    // does not itself restart anything, so it mutates nothing.
    PolicyEntry {
        path: "commands::jobs::jobs_retry",
        effect: Effect::Read,
    },
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
    PolicyEntry {
        path: "commands::ai::ai_list_models",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::ai::ai_model_capabilities",
        effect: Effect::Read,
    },
    PolicyEntry {
        path: "commands::ai::ai_inspect_model",
        effect: Effect::Read,
    },
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
    // Reclassified `Reversible` → `Irreversible` (MEDIUM fix — security
    // review round 4): the prior comment's "additive, no data destroyed"
    // answers the wrong question — `Effect::Reversible`'s own definition
    // (module doc) is "mutates state, but the change CAN BE UNDONE THROUGH
    // ANOTHER CALL ON THIS SAME SURFACE", and there is no `ai_delete_model`/
    // `ai_remove_model` (or any Ollama `/api/delete` call) ANYWHERE in this
    // crate — verified by grep, not merely absent from this file. A pulled
    // model is a real, multi-GB-scale disk write (`timeouts.rs`'s own doc:
    // "a large multi-GB… download") the app has no in-app path to reverse;
    // that is `Effect::Irreversible`'s definition ("cannot be undone through
    // the app"), not Reversible's, regardless of whether it destroys
    // anything. Not charged against the paid-provider ceiling (Ollama is
    // local/free), so this does NOT fit ADR-038's four canonical patterns —
    // classified on the general definition alone, same as
    // `system_open_external`'s "friction against an accidental/looping
    // call" reasoning: nothing scopes this ceremony to the SPECIFIC model
    // name (no id-scoped record exists to read back — the request is a bare
    // model string), so the proof is WEAK, like `ai_generate`'s spend-total
    // proof — the strongest available signal is the caller's own current
    // local model count, read fresh via `ai_list_models`, forcing at least
    // one read before every pull rather than none.
    PolicyEntry {
        path: "commands::ai::ai_pull_model",
        effect: Effect::Irreversible(ProofSource::Count {
            read_command: "ai_list_models",
        }),
    },
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
];
