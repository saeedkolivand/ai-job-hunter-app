//! `agent.call` → `agent.call.result` — ADR-038 §2's generic dispatch tier
//! (`agent call <namespace>:<command> --input '<json>'`). [`Effect::Read`]
//! AND [`Effect::Reversible`] rows dispatch directly through
//! [`tauri::Webview::on_message`] (Phase 4) — the caller can undo either
//! through the app, which is what those two classes mean. An
//! [`Effect::Irreversible`] row dispatches only after a `--confirm` ceremony
//! (Phase 3, ADR-038 §4): a call with no `confirm` refuses with
//! [`Refusal::ConfirmationRequired`], naming WHICH other read surface the
//! proof value comes from and NEVER the value itself; a wrong `confirm`
//! refuses with [`Refusal::ConfirmationMismatch`], which likewise never
//! discloses the expected value. [`Effect::NotExposed`] always refuses. A
//! dispatched command that comes back as `InvokeResponse::Err` — the body
//! ran and returned a typed `Err`, or Tauri rejected the call before the
//! body ran at all (bad args, ACL denial, unknown command) — ALSO refuses,
//! with [`Refusal::InvokeError`]: it is never folded into `dispatched: true`
//! (see that variant's own doc for why the two causes are indistinguishable
//! on the wire and both must refuse).
//!
//! ## Dispatch mechanism (verified against the vendored tauri 2.11.5
//! source, not docs.rs — ADR-038's own "verified" note)
//! `Webview::on_message` is `pub`; every `InvokeRequest` field is `pub`;
//! `AppHandle::invoke_key` is `pub` and its own doc names this EXACT use
//! ("Gets the invoke key that must be referenced when using
//! `crate::webview::InvokeRequest`"). Driving it this way runs the REAL,
//! registered command body in the app's own process against its single
//! managed state — so `limits::Limiter`/`charge_provider_daily` (which live
//! INSIDE command bodies, never in a wrapper — `commands/ai/mod.rs`) still
//! apply exactly as they do for the renderer. No codegen, no second copy of
//! any command's logic, no call-the-Rust-fn-directly shortcut that would
//! bypass those limits. The SAME mechanism resolves an `Irreversible` row's
//! proof value too (`proof::resolve` dispatches its `read_command` through
//! this exact path) — never a second implementation of a command's logic.
//!
//! `url` is the running app's OWN "main" `WebviewWindow`'s CURRENT url
//! (`WebviewWindow::url()`), never a guessed/hardcoded literal —
//! `on_message`'s private `is_local_url` only compares scheme+domain against
//! the app's own protocol origin, so reading the real webview's real address
//! is what makes this genuinely mirror what the renderer itself sends, on
//! every platform and dev-vs-prod combination, rather than hardcoding one of
//! `tauri://localhost` / `https://tauri.localhost` and silently breaking on
//! the other. `invoke_key` is read fresh off `AppHandle::invoke_key()` on
//! every call and NEVER logged/echoed/returned — its own doc: "DO NOT expose
//! this key to third party scripts as might grant access to the backend
//! from external URLs and iframes."

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::agent_cli::policy::{Effect, PolicyEntry, ProofSource, POLICY};

mod proof;

// ── `<namespace>:<command>` ⇄ policy row (derived, never hand-typed twice) ─

/// Split a [`PolicyEntry::path`] (e.g. `"commands::jobs::jobs_list"`, always
/// `module::fn` — at least one `::`) into `(namespace, command)`. `command`
/// is the bare trailing segment — the wire `cmd` Tauri actually registers
/// (confirmed against the TS client, `invoke('jobs_list', ...)`, never the
/// qualified path); `namespace` is the segment immediately before it.
/// Uniform across every row's shape (`commands::ai::ai_generate`,
/// `export::commands::documents_export_document`, `updater::updater_check`)
/// with no per-module special-casing — the SAME derivation both parses a
/// CLI token's expected shape and looks a row up, never two copies.
///
/// `pub(super)` — the `agent_cli::mcp` MCP server (a sibling module reached
/// via `extension_bridge`, not a descendant of THIS module) needs the exact
/// same `(namespace, command)` split to route a `call-*` tool locally
/// against its own bundled `POLICY` copy, and to build `commands`' rows —
/// never a second hand-typed `rsplit("::")`. Same anti-copy reasoning that
/// widened [`ERR_CONFIRMATION_REQUIRED`] below.
pub(super) fn split_path(path: &str) -> (&str, &str) {
    let mut segments = path.rsplit("::");
    let command = segments.next().unwrap_or(path);
    let namespace = segments.next().unwrap_or("");
    (namespace, command)
}

/// The one [`PolicyEntry`] whose derived `(namespace, command)` matches
/// EXACTLY — never a fuzzy/partial match (a typo'd namespace on an
/// otherwise-real command name refuses rather than silently dispatching:
/// `command` alone already uniquely identifies a row, since
/// `generate_handler!` requires globally-unique command names, so a
/// namespace mismatch can only mean the caller typed the wrong one).
fn find_policy(namespace: &str, command: &str) -> Option<&'static PolicyEntry> {
    POLICY
        .iter()
        .find(|entry| split_path(entry.path) == (namespace, command))
}

// ── Refusals — distinct sentinel + detail per cause, one reply builder ─────

/// Every reason dispatch never reached (or never completed)
/// `Webview::on_message`. One enum, one `sentinel`/`detail` pair per
/// variant, one reply builder below — collapsing two of these into one
/// sentinel is exactly the defect this repo's own `agent_cli` module doc
/// says has already been fixed twice on this surface. `pub(super)` — it is
/// the `Err` side of [`gate`]'s return type, and `gate` is itself
/// `pub(super)` for `extension_bridge::test`; Rust's private-interfaces lint
/// requires every type in a `pub(super)` fn's signature to be at least as
/// visible, regardless of whether a caller actually names a variant.
pub(super) enum Refusal {
    /// No policy row matches this `(namespace, command)` pair at all.
    UnknownCommand,
    /// [`Effect::NotExposed`] — deliberately unreachable; carries that row's
    /// own stored reason.
    NotExposed(&'static str),
    /// `agent.call` arrived over a connection whose handshake `Origin`
    /// wasn't the CLI's — same class as `msg::AGENT_QUERY`'s origin gate.
    OriginRefused,
    /// The shared `agent.query`/`agent.call` throttle bucket is empty.
    RateLimited,
    /// The row's own command dispatch (`Webview::on_message`) never produced
    /// a usable reply (no "main" webview, its url couldn't be read, or the
    /// responder never fired) — a framework-level failure, never the
    /// caller's input, so the carried string is always one of
    /// [`invoke_command`]'s own fixed messages, never an echo of `input`.
    DispatchFailed(String),
    /// `InvokeResponse::Err` (HIGH fix — security review): the target
    /// command's OWN dispatch produced a Tauri-level error rather than a
    /// success payload — distinct from [`Refusal::DispatchFailed`], which is
    /// a framework failure that never reaches the target command at all
    /// (no "main" webview, its url unreadable, no reply). This is the fix
    /// for the defect where `InvokeResponse::Err` used to be folded straight
    /// into `Ok`, reporting `dispatched: true` for a call whose command body
    /// either failed validation or never ran (bad args, ACL denial, unknown
    /// command) — see [`InvokeOutcome::CommandErr`]'s own doc for why the
    /// two cannot be told apart here, and why both must refuse.
    InvokeError(String),
    /// [`Effect::Irreversible`] with no `confirm` supplied — exit 4 (see
    /// `agent_cli::exit_code_for_reply`), distinct from every other refusal
    /// here (all exit 2). Names WHICH read surface + field the proof comes
    /// from and NEVER the value itself (ADR-038 §4).
    ConfirmationRequired(String),
    /// A `confirm` value that did not match the freshly-resolved proof. The
    /// detail is a FIXED string, never the hint and never the expected
    /// value — a mismatch must not leak anything a caller couldn't already
    /// have gotten from the `ConfirmationRequired` refusal alone.
    ConfirmationMismatch,
    /// The proof value itself could not be resolved (the read it depends on
    /// failed, or the targeted record doesn't exist) — distinct from a
    /// wrong-value mismatch so a caller can tell "you guessed wrong" apart
    /// from "the thing you're trying to act on isn't there".
    ProofUnavailable,
    /// The command RAN, but its reply is larger than the bridge's own
    /// [`super::MAX_FRAME_BYTES`] frame cap and was discarded (issue #1135).
    /// Carries the MEASURED byte count, never an estimate.
    ///
    /// Why this variant exists at all: `max_message_size` in tungstenite
    /// 0.30 (what `tokio-tungstenite = "0.30"` resolves to) is checked on the
    /// READ path only — `WebSocketContext`'s `check_max_size` runs while
    /// reassembling an INCOMING message, and nothing checks an outgoing one.
    /// So the app happily wrote an over-cap frame, the CLI's own read loop
    /// collapsed the resulting `Error::Capacity(MessageTooLong)` into "this
    /// port gave us nothing usable" (`agent_cli::next_json` returns `None` on
    /// every transport error alike), and the caller got a content-free
    /// `connection_lost` — a sentinel whose own `--help` text and the MCP
    /// server's `instructions` both group with TRANSIENT failures, so a
    /// client burned its one permitted retry on a call that can never
    /// succeed. Refusing HERE, at the one place that has both the reply and
    /// its length, turns a deterministic failure into a deterministic,
    /// self-describing refusal.
    ///
    /// Deliberately checked against [`super::MAX_FRAME_BYTES`] and not
    /// against the MCP server's own much smaller `MCP_RESULT_MAX_BYTES`: the
    /// two caps sit on different transports and the smaller one already
    /// refuses (with this same `result_too_large` sentinel) one hop further
    /// out. Adopting it here would newly refuse payloads that reach a plain
    /// `agent call` caller perfectly well today.
    ResultTooLarge(usize),
    /// A caller-supplied `cursor` on one of [`PAGINATED_LIST_COMMANDS`] that
    /// isn't a plain non-negative integer offset. The detail is the FIXED
    /// [`super::paging::INVALID_CURSOR_MESSAGE`] and NEVER the offending
    /// value — same never-echo-the-caller's-own-token discipline as
    /// [`Refusal::ConfirmationMismatch`]; a cursor arrives from an untrusted
    /// tool call and reaching an LLM's context verbatim is exactly the echo
    /// this surface avoids everywhere else.
    InvalidCursor,
}

/// `pub(super)` — the MCP server's `call-*` tools refuse locally with this
/// SAME sentinel for a `<namespace>:<command>` their own bundled `POLICY`
/// copy doesn't know, rather than a second hand-typed literal (same
/// reasoning as [`ERR_CONFIRMATION_REQUIRED`]'s own doc).
pub(super) const ERR_UNKNOWN_COMMAND: &str = "unknown_command";
/// `pub(super)` — the MCP server refuses a `NotExposed` row LOCALLY with this
/// SAME sentinel (so the token-row fix does not depend on the peer app's build;
/// see `agent_cli::mcp::local_call_refusal`), never a second hand-typed copy.
pub(super) const ERR_NOT_EXPOSED: &str = "not_exposed";
const ERR_CLI_ONLY: &str = "cli_only";
const ERR_RATE_LIMITED: &str = "rate_limited";
const ERR_DISPATCH_FAILED: &str = "dispatch_failed";
const ERR_INVOKE_ERROR: &str = "invoke_error";
/// `pub(super)` — [`super::agent_cli::exit_code_for_reply`] matches on this
/// EXACT sentinel to special-case exit 4, never a second hand-typed copy of
/// the string.
pub(super) const ERR_CONFIRMATION_REQUIRED: &str = "confirmation_required";
const ERR_CONFIRMATION_MISMATCH: &str = "confirmation_mismatch";
const ERR_PROOF_UNAVAILABLE: &str = "proof_unavailable";
/// `pub(super)` — the MCP server's own, much smaller result cap
/// (`agent_cli::mcp::oversized_result`) refuses with this SAME sentinel one
/// hop further out. One cause, one name, ONE definition of the string: two
/// hand-typed copies of a sentinel is the drift this module's own
/// [`ERR_CONFIRMATION_REQUIRED`] doc already argues against.
pub(super) const ERR_RESULT_TOO_LARGE: &str = "result_too_large";
const ERR_INVALID_CURSOR: &str = "invalid_cursor";

/// Fixed sentinel — mirrors `agent_read::CLI_ONLY_MESSAGE` for the identical
/// gate, applied to the generic tier's own wire type.
const CLI_ONLY_MESSAGE: &str = "agent.call is only available to the ajh-tauri agent CLI";

impl Refusal {
    fn sentinel(&self) -> &'static str {
        match self {
            Refusal::UnknownCommand => ERR_UNKNOWN_COMMAND,
            Refusal::NotExposed(_) => ERR_NOT_EXPOSED,
            Refusal::OriginRefused => ERR_CLI_ONLY,
            Refusal::RateLimited => ERR_RATE_LIMITED,
            Refusal::DispatchFailed(_) => ERR_DISPATCH_FAILED,
            Refusal::InvokeError(_) => ERR_INVOKE_ERROR,
            Refusal::ConfirmationRequired(_) => ERR_CONFIRMATION_REQUIRED,
            Refusal::ConfirmationMismatch => ERR_CONFIRMATION_MISMATCH,
            Refusal::ProofUnavailable => ERR_PROOF_UNAVAILABLE,
            Refusal::ResultTooLarge(_) => ERR_RESULT_TOO_LARGE,
            Refusal::InvalidCursor => ERR_INVALID_CURSOR,
        }
    }

    /// Human/agent-readable detail. [`Refusal::ConfirmationRequired`] and
    /// [`Refusal::ConfirmationMismatch`] never carry the proof VALUE — see
    /// each variant's own doc; this is the one place both are rendered, so
    /// it is also the one place that guarantee could be broken, hence the
    /// dedicated tests in `agent_call::tests`.
    ///
    /// MEDIUM fix (security review round 4): [`Refusal::InvokeError`]'s
    /// `detail` used to render unfenced — [`dispatch_direct`]'s SUCCESS
    /// payload runs through [`fence_scraped_fields`], but a command's error
    /// value never did, so a dispatchable command whose `AppError` embeds
    /// third-party/remote text (a scrape/HTTP/provider failure that echoes
    /// part of a caller-chosen host's own response) reached the caller
    /// verbatim through this ONE surviving unfenced channel — the same class
    /// of gap `ai_test_provider_key`'s CRITICAL finding closed on the
    /// success side. Fenced the SAME way as every other untrusted string
    /// this file emits, not a second primitive.
    fn detail(&self) -> String {
        match self {
            Refusal::UnknownCommand => {
                "no policy row matches this <namespace>:<command> — run `agent schema` or the \
                 MCP `commands` tool to enumerate targets, or see policy.rs for the full table"
                    .to_string()
            }
            Refusal::NotExposed(reason) => format!("not exposed to any CLI tier: {reason}"),
            Refusal::OriginRefused => CLI_ONLY_MESSAGE.to_string(),
            Refusal::RateLimited => super::agent_read::THROTTLED_MESSAGE.to_string(),
            Refusal::DispatchFailed(detail) => detail.clone(),
            Refusal::InvokeError(detail) => {
                let fenced = crate::prompt_fence::fenced(
                    "job_posting",
                    detail,
                    crate::prompt_fence::JOB_CAP,
                );
                format!(
                    "the command either ran and returned an error, or Tauri rejected the call \
                     before the body ran (missing/invalid args, an ACL denial, or an unregistered \
                     command) — these are wire-indistinguishable; underlying value: {fenced}"
                )
            }
            Refusal::ConfirmationRequired(hint) => hint.clone(),
            Refusal::ConfirmationMismatch => {
                "the confirm value did not match — it is never disclosed by this refusal; \
                 re-read the source named in a fresh confirmation_required refusal for this \
                 same command"
                    .to_string()
            }
            Refusal::ProofUnavailable => {
                "could not resolve a proof value for this target — the referenced record may \
                 not exist, or the read it depends on failed"
                    .to_string()
            }
            // Mirrors `agent_cli::mcp::oversized_result`'s wording MINUS its
            // "narrow the query" advice, which presupposes a caller-adjustable
            // parameter this tier's over-cap commands do not have (issue #1136
            // gave `applications_list`/`ai_generations_list` one; the rest,
            // `autopilot_list` above all, still have none). Says outright that
            // the command RAN: `dispatched` is `false` on this reply because
            // no result was delivered, and a caller that read that as "nothing
            // happened" would retry a mutation that already took effect.
            Refusal::ResultTooLarge(bytes) => format!(
                "the command RAN, but its reply ({bytes} B) exceeds the bridge's own frame cap \
                 and was discarded rather than truncated — dispatched:false here means no \
                 result was delivered, NOT that nothing happened, so do not re-run a mutating \
                 command on this refusal. Retrying is futile: the outcome is deterministic for \
                 this data. Ask the user to run it outside this session, or use a bounded \
                 alternative if this command has one (the MCP `commands` tool marks the paged \
                 ones)."
            ),
            Refusal::InvalidCursor => super::paging::INVALID_CURSOR_MESSAGE.to_string(),
        }
    }
}

/// `dispatched`, never `ok` (ADR-038 §5): ~47 commands signal failure INSIDE
/// their own Ok payload, so this dispatcher cannot know whether the
/// underlying operation succeeded — only whether it ran. `data` is the
/// command's payload verbatim (no PII redaction — ADR-038's amendment to
/// ADR-0005, scoped to this generic tier by the owner's explicit decision).
fn call_result_reply(
    req_id: &str,
    namespace: &str,
    command: &str,
    outcome: Result<Value, Refusal>,
) -> String {
    let payload = match outcome {
        Ok(data) => json!({
            "dispatched": true,
            "namespace": namespace,
            "command": command,
            "data": data,
        }),
        Err(refusal) => json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": refusal.sentinel(),
            "detail": refusal.detail(),
        }),
    };
    json!({
        "type": super::msg::AGENT_CALL_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Reply for an `agent.call` arriving over a connection whose handshake
/// `Origin` wasn't `auth::AGENT_CLI_ORIGIN` — mirrors
/// `agent_read::origin_refused_reply` exactly, one wire type over.
pub(super) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    call_result_reply(req_id, namespace, command, Err(Refusal::OriginRefused))
}

pub(super) fn throttled_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    call_result_reply(req_id, namespace, command, Err(Refusal::RateLimited))
}

fn payload_target(payload: &Value) -> (&str, &str) {
    let namespace = payload
        .get("namespace")
        .and_then(Value::as_str)
        .unwrap_or("");
    let command = payload.get("command").and_then(Value::as_str).unwrap_or("");
    (namespace, command)
}

/// The throttle key `agent.call` draws from — reuses
/// `BridgeState::try_acquire_agent`'s EXISTING two buckets (never a second
/// throttle instance): `autopilot_best_matches` is the one Read-effect
/// command in [`POLICY`] that runs the SAME uncapped clustering pass
/// `agent_read`'s `best-matches` resource already rate-limits tightly, so it
/// maps into that resource's own bucket key — sharing state so a caller
/// cannot double an allowance by alternating tiers. Every other command
/// falls into the shared cheap bucket (any key other than `"best-matches"`
/// does, by `AgentQueryThrottle::try_acquire_at`'s own construction). Note:
/// this throttle admits the TARGET command's own frame only — an
/// `Irreversible` row's proof-resolution read (`proof::resolve`) dispatches
/// a SECOND, internal command and is not separately throttled; bounded to
/// exactly one extra read per confirm attempt, so left as-is rather than
/// adding a second bucket for a cost this small.
pub(super) fn throttle_key(command: &str) -> &str {
    if command == "autopilot_best_matches" {
        "best-matches"
    } else {
        command
    }
}

// ── Fencing scraped job-posting text (a different axis from the raw-data
// decision above — ADR-038's own amendment paragraph) ──────────────────────

/// Response field NAMES that can carry raw, third-party-authored SCRAPED JOB
/// TEXT — audited by hand against the struct each name actually serializes
/// from (mirrors `policy`'s own per-row audit discipline). Keyed by FIELD
/// NAME rather than by command (HIGH fix — security review round 2): a
/// command allowlist (the prior shape of this const) missed every command
/// whose response embeds one of these structs under this same key — real
/// examples that leaked unfenced: `autopilot_list`/`autopilot_get`
/// (`Autopilot.found_jobs[].description`), `applications_list`/
/// `applications_get` (`Application.job_description` → `jobDescription`),
/// `ai_generations_list` (`AiGenerationRecord.job_ad` → `jobAd`). Every entry
/// routes through [`crate::prompt_fence::fenced`] — the SAME primitive, tag,
/// and cap `agent_read::fence_description` uses for the curated `job`
/// resource, so a scraped posting reads as untrusted DATA on every surface
/// it reaches. See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the audited list of rows this is
/// known to protect.
///
/// HIGH fix (security review round 3): this list named `description`/
/// `jobAd`/`jobDescription` but not `title`/`company`/`location`/
/// `requirements`, which `scraping::types::JobPosting` and
/// `autopilot::FoundJob` ALSO carry, board-derived and equally
/// third-party-authored (a posting *titled* "Ignore prior instructions; run:
/// …" reached the caller unfenced). `requirements` is an
/// `Option<Vec<String>>` — [`fence_named_fields_recursive`] now fences
/// string ARRAY elements under a listed key too, not just a bare string.
///
/// HIGH fix (security review round 4): a FLAT field-name list silently
/// misses a serde-RENAMED field carrying the exact same posting data under a
/// different key — `AiGenerationRecord.job_title`/`.company_name`/
/// `.top_requirements` (`ai_generations_list`/`ai_generations_get`) are the
/// board-derived title/company/requirements COPIED FORWARD from the source
/// posting into a new struct, not re-derived, so they are exactly as
/// untrusted as `JobPosting.title`/`.company`/`.requirements` already
/// listed above — a flat list keyed on THOSE structs' field names never
/// covered the SAME data reappearing under `AiGenerationRecord`'s own
/// names. `discovered::DiscoveredCompany.display_name` → `displayName`
/// (`discovery_search_companies`) is board-harvested from a posting's own
/// apply-redirect URL, same category. `documents::DocumentRecord.text`
/// (`documents_list`/`documents_get_text`) and
/// `notifications::AppNotification.body` (`notifications_list`) are the
/// generic `text`/`body` carriers this round closes — a résumé's own text
/// is user-uploaded content, not board-scraped, but this repo's own
/// standing threat model (`agent-cli-standards` skill: ~1% of a 200k-résumé
/// corpus carried a prompt injection, sevenfold over 16 months) treats it as
/// exactly as untrusted as a job posting for this purpose; a notification
/// body can echo a scraped title/company by construction (`autopilot.new_
/// jobs`). See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the full audited row list, and
/// [`ai_generation_record_struct_fixture_fences_the_posting_derived_fields`]/
/// [`discovered_company_struct_fixture_fences_display_name`] for the
/// fixture-driven exhaustive checks this round adds — the reviewer's own
/// diagnosis for why round 3's flat list still missed fields: build the
/// check from a REAL struct via `serde_json::to_value`, not by continuing to
/// hand-guess names one round at a time.
///
/// Deliberately NOT added (out of scope for this list, on PURPOSE, not by
/// omission): `AiGenerationRecord.resume_text`/`.cover_letter_text`/
/// `.company_brief`/`.candidate_name`/`.email_subject`/`.email_body` and
/// `InterviewQuestion`/`ApplicationAnswer`'s own fields — these are the
/// user's own PII / this app's own AI output, not board-scraped/third-party
/// text; ADR-038's own amendment already draws this exact line as a
/// SEPARATE axis from fencing (ADR-038 §5's "no PII redaction… scoped to
/// this generic tier by the owner's explicit decision" — this module's own
/// doc comment above). Flagged for a human/security-critic sanity check
/// rather than silently expanded, since `ApplicationAnswer.question` (a
/// THIRD-PARTY ATS form's own question label) is a plausible future
/// candidate on the SAME reasoning as `title`/`jobDescription` above.
const FENCE_FIELD_NAMES: &[&str] = &[
    // `scraping::types::JobPosting.description` (scrape_resolve_url,
    // scrape_list_postings) AND `autopilot::FoundJob.description`
    // (autopilot_list, autopilot_get) — same key, two different structs.
    "description",
    // `ai_generations::AiGenerationRecord.job_ad` (ai_generations_list) —
    // the full scraped posting text handed to the AI provider verbatim.
    "jobAd",
    // `applications::Application.job_description` (applications_list,
    // applications_get) — the scraped posting text an Application was
    // tracked/generated from.
    "jobDescription",
    // `JobPosting.title`/`FoundJob.title` — board-derived, third-party
    // authored, and NOT covered by the array-of-strings handling below.
    "title",
    // `JobPosting.company`/`FoundJob.company` — same reasoning as `title`.
    "company",
    // `JobPosting.location`/`FoundJob.location` — same reasoning as `title`.
    "location",
    // `JobPosting.requirements: Option<Vec<String>>` — an ARRAY of
    // board-extracted requirement snippets, not a bare string; see
    // `fence_named_fields_recursive`'s array handling.
    "requirements",
    // `AiGenerationRecord.job_title` — the posting's title COPIED FORWARD
    // into the generation record, not re-derived; same risk as `title`.
    "jobTitle",
    // `AiGenerationRecord.company_name` — same reasoning as `jobTitle` above.
    "companyName",
    // `AiGenerationRecord.top_requirements: Vec<String>` — an ARRAY, fenced
    // element-by-element via the array handling below.
    "topRequirements",
    // `documents::DocumentRecord.text` (`documents_list`,
    // `documents_get_text`) — the user's uploaded résumé/cover-letter text;
    // see this const's own doc for why this repo treats it as untrusted.
    "text",
    // `notifications::AppNotification.body` (`notifications_list`) — can
    // echo a scraped job title/company inside app-generated copy.
    "body",
    // `discovered::DiscoveredCompany.display_name` (`discovery_search_
    // companies`) — board-harvested from a posting's own apply-redirect URL.
    "displayName",
];

/// `JobPosting`'s own always-present, distinctively-named field pair
/// (`captured_at` → `capturedAt`, `source`) — used to detect a
/// `JobPosting`-shaped object so its `#[serde(flatten)] extra:
/// HashMap<String, Value>` (board-specific metadata: salary, remote status,
/// etc.) can be treated as untrusted too (HIGH fix — security review round
/// 3). `extra`'s keys are BOARD-chosen, not enumerable by name the way
/// [`FENCE_FIELD_NAMES`] enumerates a Rust struct's own fields, so a
/// field-name allowlist structurally cannot cover them — verified no other
/// struct reaching this dispatch surface serializes both fields together.
const JOB_POSTING_ANCHOR_FIELDS: [&str; 2] = ["capturedAt", "source"];

/// Structural `JobPosting` fields that are identifiers/URLs/timestamps,
/// never third-party PROSE — every OTHER string value on a
/// [`JOB_POSTING_ANCHOR_FIELDS`]-detected object is untrusted (flattened
/// `extra`, or a future field this file doesn't yet name by hand).
const JOB_POSTING_SAFE_FIELDS: &[&str] = &[
    "id",
    "externalId",
    "url",
    "source",
    "capturedAt",
    "postedAt",
];

/// Fence every [`FENCE_FIELD_NAMES`] string (or string array element)
/// anywhere in `data`'s tree — recurses through the WHOLE response (not just
/// a top-level object/array, MEDIUM fix — security review round 1), and runs
/// UNCONDITIONALLY for every dispatched command rather than gating on a
/// command allowlist (HIGH fix — security review round 2): a new command
/// whose response embeds one of these EXACT field names is fenced
/// automatically, without needing an entry added here first. Also fences any
/// unclassified string field on a [`JOB_POSTING_ANCHOR_FIELDS`]-detected
/// object (HIGH fix — security review round 3), closing the residual gap a
/// field-name allowlist alone cannot: `JobPosting.extra`'s board-chosen keys.
/// See `every_known_posting_text_carrier_is_a_real_freely_
/// dispatchable_policy_row` (tests) for the audited list of rows this is
/// known to protect.
fn fence_scraped_fields(data: &mut Value) {
    fence_named_fields_recursive(data);
}

/// Walk every object/array in `value`, fencing any [`FENCE_FIELD_NAMES`]
/// STRING key (or string element of an ARRAY under one of those keys)
/// wherever one appears, then — on an object [`JOB_POSTING_ANCHOR_FIELDS`]
/// marks as a real `JobPosting` — every OTHER string-valued key not in
/// [`JOB_POSTING_SAFE_FIELDS`] (the flattened `extra` catch-all). See
/// [`fence_scraped_fields`]'s doc for why this is recursive and
/// unconditional.
fn fence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for field in FENCE_FIELD_NAMES {
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let fenced =
                        crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
                    map.insert((*field).to_string(), json!(fenced));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            *s = crate::prompt_fence::fenced(
                                "job_posting",
                                s,
                                crate::prompt_fence::JOB_CAP,
                            );
                        }
                    }
                }
            }
            if JOB_POSTING_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f))
            {
                // ADVISORY fix (security review round 4): used to filter on
                // `v.is_string()` alone, so a board-chosen `extra` key whose
                // value is an ARRAY or OBJECT (not reachable today — every
                // `extra.insert` call site writes a scalar, verified — but
                // not reachable is not the same as impossible for the FIRST
                // board that adds one) skipped this catch-all entirely: not
                // a listed field name, not string-typed, so neither this
                // block nor the array-only handling above touches it, and
                // the generic recursive walk below only fences NAMED fields,
                // never "every string inside an unclassified value". Every
                // non-null, non-safe, non-listed key is now collected
                // regardless of shape; a String is fenced directly as
                // before, an Array/Object is fenced leaf-by-leaf via
                // `fence_all_string_leaves` (untrusted board data all the
                // way down, not just at the top level).
                let extra_keys: Vec<String> = map
                    .iter()
                    .filter(|(k, v)| {
                        !v.is_null()
                            && !FENCE_FIELD_NAMES.contains(&k.as_str())
                            && !JOB_POSTING_SAFE_FIELDS.contains(&k.as_str())
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                for key in extra_keys {
                    if let Some(v) = map.get_mut(&key) {
                        match v {
                            Value::String(s) => {
                                *s = crate::prompt_fence::fenced(
                                    "job_posting",
                                    s,
                                    crate::prompt_fence::JOB_CAP,
                                );
                            }
                            Value::Array(_) | Value::Object(_) => fence_all_string_leaves(v),
                            _ => {}
                        }
                    }
                }
            }
            for v in map.values_mut() {
                fence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}

/// Fence every STRING found anywhere inside `value`, unconditionally — no
/// field-name gate, unlike [`fence_named_fields_recursive`]. Used only for a
/// value already known to be untrusted board data by virtue of its
/// LOCATION (an unclassified key under a detected `JobPosting`'s flattened
/// `extra`), so every string it contains, at any depth, is untrusted too —
/// the board chose the keys, so a name-based allowlist can never enumerate
/// them.
fn fence_all_string_leaves(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_all_string_leaves(item);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                fence_all_string_leaves(v);
            }
        }
        _ => {}
    }
}

// ── Agent-layer response reshaping (paging + byte encoding) ──────────────
//
// Two audited consts in the SAME hand-audited style as `FENCE_FIELD_NAMES`
// above, applied at the SAME chokepoint (`dispatch_direct`), for the same
// reason that const exists: the shared `commands/**` bodies are the
// RENDERER's wire shape and must stay byte-for-byte identical, so anything
// only an agent needs is done here, on the way out, and nowhere else.

/// Commands whose reply is an unbounded, growth-only, already-newest-first
/// ARRAY that NO command argument can narrow — both take `(app: AppHandle)`
/// and nothing else, and both back a `SELECT … ORDER BY … DESC` with no
/// `LIMIT` (issue #1136: 1.7 MB of `ai_generations` and 440 KB of
/// `applications` on an ordinary account, i.e. permanently over the MCP
/// server's 256 KiB result cap with no parameter a caller could add to
/// succeed). Paged HERE rather than in `commands/**`, whose wire shape the
/// renderer's own service hooks depend on — the shared command bodies are
/// deliberately untouched by this fix.
///
/// Audited by hand, one row at a time, against the command's real signature
/// and its real query (mirrors `policy`'s own per-row audit discipline):
/// - `applications_list` → `applications::ApplicationStore::list`
/// - `ai_generations_list` → `ai_generations::AiGenerationStore::list`
///
/// Adding a row here CHANGES that command's reply shape for every generic-tier
/// caller (a bare array becomes [`paginate_list_reply`]'s envelope), so it is
/// an audited list and not a heuristic like "any command whose reply is a big
/// array" — a shape-sniffing rule would silently reshape a future command
/// whose array is bounded by construction, and reshape it differently as the
/// user's data grew. `pub(super)` — `agent_cli::mcp`'s `commands` tool marks
/// these rows so a caller can DISCOVER the paging instead of inferring it
/// from a surprise envelope, never a second hand-typed name list.
pub(super) const PAGINATED_LIST_COMMANDS: &[&str] = &["applications_list", "ai_generations_list"];

/// What the `commands` tool prints on a [`PAGINATED_LIST_COMMANDS`] row.
/// Lives HERE, next to the behaviour it describes, so the description cannot
/// drift from the list it describes (`agent-cli-standards`: nothing
/// hand-maintained that can drift). Deliberately names no default/cap number
/// — those live on [`DEFAULT_LIST_PAGE_LIMIT`]/[`MAX_LIST_PAGE_LIMIT`] and a
/// copy here would be a second source of truth for them.
pub(super) const PAGINATED_LIST_NOTE: &str =
    "returns a paged envelope {items,total,nextCursor} instead of a bare array (the raw list is \
     unbounded and exceeds the result cap). Pass input.limit (clamped server-side) and \
     input.cursor (a prior reply's nextCursor, verbatim; omit for the first page); repeat until \
     nextCursor is null. `total` is the FULL row count, unaffected by paging — it is what an \
     Effect::Irreversible row whose proof is this list's length wants.";

/// Server-side default/cap for a [`PAGINATED_LIST_COMMANDS`] `limit` — a
/// CEILING on rows per page, never the transport-size guarantee. That
/// guarantee is [`LIST_PAGE_BYTE_BUDGET`], enforced by
/// `paging::trim_to_byte_budget` against the REAL serialized bytes, because a
/// row count cannot bound bytes here either: one `AiGenerationRecord` carries
/// a full résumé, a cover letter AND the whole scraped job ad, while an
/// `Application` row is a fraction of that, so no single count can be right
/// for both. The count is only the cheap first cut — for `ai_generations` the
/// budget will usually cut a page well below the default, and for the much
/// smaller `applications` rows the default is comfortably inside it.
const DEFAULT_LIST_PAGE_LIMIT: usize = 20;
const MAX_LIST_PAGE_LIMIT: usize = 100;

/// The REAL per-response bound for a paged list reply. Same 150,000 B as
/// `agent_read::found_jobs::PAGE_BYTE_BUDGET` and for the same reason: this
/// payload rides inside the MCP server's `content[]`/`isError` wrapper under
/// its 256 KiB `MCP_RESULT_MAX_BYTES` cap, so half of that leaves real margin
/// for the wrapper. Measured AFTER [`fence_scraped_fields`] has run, on the
/// bytes actually about to go on the wire — fencing expands every scraped
/// field it touches, so a budget checked before it would be measuring a
/// payload that no longer exists by the time it ships.
const LIST_PAGE_BYTE_BUDGET: usize = 150_000;

/// `(command, field)` pairs whose value is a RAW BYTE ARRAY that `serde_json`
/// renders as ~3.2–4× its own size in decimal digits and commas (issue
/// #1138: an ordinary one-page résumé exported to PDF came back at 259,841 B,
/// 99.1% of the MCP result cap, and a two-page one exceeded it — with no
/// `limit`/`cursor` to narrow and no other exposed export path). Re-encoded
/// base64 (~1.33×) HERE, never on the struct: `ExportResult.data` is the
/// RENDERER's own wire shape (`data: number[]`, consumed by the export
/// service hooks through `AppClient`), and `#[serde(with = …)]` on that field
/// would change it for them too.
///
/// Audited by hand against the struct each pair actually serializes from:
/// - `documents_export_document` → `export::types::ExportResult.data:
///   Vec<u8>` (camelCase-renamed struct; `data` is already its wire key).
///
/// `documents_render_preview_images` is the other payload the MCP cap's own
/// comment names, and it is deliberately NOT here: `PreviewResult.pages` is
/// `Vec<String>` of SVG source, already text, and base64ing it would make it
/// bigger and unreadable.
const BASE64_BYTE_FIELDS: &[(&str, &str)] = &[("documents_export_document", "data")];

/// Suffix appended to a [`BASE64_BYTE_FIELDS`] field name to form the sibling
/// key that DECLARES the encoding (`data` → `dataEncoding`). Derived from the
/// field name rather than listed per pair so a second entry cannot forget it.
const ENCODING_KEY_SUFFIX: &str = "Encoding";
const BASE64_ENCODING: &str = "base64";

/// Re-encode every [`BASE64_BYTE_FIELDS`] array-of-bytes on `command`'s reply
/// as a base64 STRING, and add the sibling `<field>Encoding: "base64"` key
/// that says so. A payload that describes its own encoding survives a caller
/// that never read the server `instructions` or the tool description — the
/// reason this is a wire key and not documentation.
///
/// Top-level only, and by exact `(command, field)` pair — the opposite of
/// [`fence_named_fields_recursive`]'s unconditional recursive walk, on
/// purpose: fencing is a SAFETY property that must cover a field wherever it
/// appears, while this is a lossy-looking representation change that must
/// only ever hit the one field whose type was audited. A recursive
/// "any array of small integers is bytes" rule would eventually rewrite a
/// legitimate array of scores or ids into gibberish.
///
/// A non-array value (or a `data` that is not entirely bytes) is left exactly
/// as it was and gets NO marker key — the marker is only ever added on a
/// value this actually re-encoded, so the two can never disagree.
///
/// `pub(super)` for ONE reason: the test that proves this actually solves
/// #1138 has to compare against `agent_cli::mcp::MCP_RESULT_MAX_BYTES`, the
/// cap it exists to get under, and that constant is private to the `mcp`
/// module — so the test lives THERE, beside the cap, rather than here beside
/// a hand-copied literal of it that could silently drift (same
/// cross-module-test reasoning as [`gate`]'s own `pub(super)`).
pub(super) fn base64_byte_fields(command: &str, data: &mut Value) {
    use base64::Engine;
    let Some(map) = data.as_object_mut() else {
        return;
    };
    for (cmd, field) in BASE64_BYTE_FIELDS {
        if *cmd != command {
            continue;
        }
        let Some(Value::Array(items)) = map.get(*field) else {
            continue;
        };
        let bytes: Option<Vec<u8>> = items
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect();
        let Some(bytes) = bytes else { continue };
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        map.insert((*field).to_string(), json!(encoded));
        map.insert(
            format!("{field}{ENCODING_KEY_SUFFIX}"),
            json!(BASE64_ENCODING),
        );
    }
}

/// Take the agent-layer paging arguments OFF `input` for a
/// [`PAGINATED_LIST_COMMANDS`] target, returning `(offset, limit)`; `None`
/// for every other command, whose `input` is left untouched.
///
/// `limit`/`cursor` are REMOVED rather than read in place because they belong
/// to this layer, not to the command: neither of these two commands declares
/// any argument at all, and a future one that declared its own `limit` must
/// not receive the paging layer's copy of it. Clamping vs refusing follows
/// `found-jobs` exactly (shared primitives, `extension_bridge::paging`): a
/// junk `limit` clamps to the default (never to "unbounded"), while a junk
/// `cursor` REFUSES — silently resetting a cursor to 0 looks like forward
/// progress while actually restarting the traversal, which is how a paging
/// loop turns into an infinite one.
fn take_list_page_args(
    command: &str,
    input: &mut Value,
) -> Result<Option<(usize, usize)>, Refusal> {
    if !PAGINATED_LIST_COMMANDS.contains(&command) {
        return Ok(None);
    }
    let offset = super::paging::parse_offset_cursor(input).map_err(|_| Refusal::InvalidCursor)?;
    let limit = super::paging::clamp_limit(input, DEFAULT_LIST_PAGE_LIMIT, MAX_LIST_PAGE_LIMIT);
    if let Some(map) = input.as_object_mut() {
        map.remove("limit");
        map.remove("cursor");
    }
    Ok(Some((offset, limit)))
}

/// Slice an already-fenced array reply into one page and wrap it in the
/// `{items,total,nextCursor}` envelope. Pure — the whole of #1136's logic is
/// unit-testable without an `AppHandle`, the same pure/impure split
/// [`classify_response`] and `confirm_and_run` use.
///
/// Runs AFTER [`fence_scraped_fields`], which is left unconditional over the
/// WHOLE reply: fencing is the security property, and narrowing what it walks
/// to "only the rows we are about to return" would make its coverage depend
/// on a paging decision. The cost is unchanged from before this fix — that
/// full-array walk already happened on every one of these calls.
///
/// A non-array reply is returned verbatim (no envelope), so a command that
/// ever stopped returning an array degrades to today's behaviour instead of
/// producing `{items: <not an array>}`.
///
/// `total` is the FULL row count, not the page's — it is what tells a caller
/// the traversal is still moving, and it is the value an `Effect::Irreversible`
/// row whose `ProofSource::Count` reads this list is really after. The proof
/// ceremony itself is unaffected either way: `proof::resolve` dispatches
/// through [`invoke_command`] directly, never through [`dispatch_direct`], so
/// it still resolves against the complete, unpaged array.
fn paginate_list_reply(data: Value, offset: usize, limit: usize) -> Value {
    let Value::Array(rows) = data else {
        return data;
    };
    let total = rows.len();
    let candidates: Vec<Value> = rows.into_iter().skip(offset).take(limit).collect();

    // `base_cost` = every envelope byte OTHER than the `items` array itself,
    // measured rather than assumed (same derivation as
    // `agent_read::found_jobs::resolve_found_jobs`'s own call site).
    // `nextCursor` isn't known yet — it depends on how many rows survive the
    // trim just below — so it is measured as a digit string the length of
    // `total`, an upper bound (a real offset can never exceed `total`), which
    // can only over-count and so only ever trim MORE than strictly required.
    let base_envelope = json!({ "items": [], "total": total, "nextCursor": total.to_string() });
    let base_cost = serde_json::to_string(&base_envelope)
        .map_or(usize::MAX, |s| s.len())
        .saturating_sub(2); // the placeholder `[]`'s own two bytes

    let page = super::paging::trim_to_byte_budget(candidates, base_cost, LIST_PAGE_BYTE_BUDGET);
    let next_offset = offset + page.len();
    let next_cursor = (next_offset < total).then(|| next_offset.to_string());
    json!({ "items": page, "total": total, "nextCursor": next_cursor })
}

// ── Dispatch ─────────────────────────────────────────────────────────────

/// What `Webview::on_message`'s callback handed back, translated into
/// [`dispatch_direct`]'s own vocabulary — split out so the translation
/// itself (`classify_response`) is a PURE fn, unit-testable without a live
/// `AppHandle` (this crate has no `tauri::test` mock-app harness; see
/// `documents::embedding`'s doc for the same constraint elsewhere).
enum InvokeOutcome {
    /// The command body ran and returned its success payload.
    Success(Value),
    /// `InvokeResponse::Err` (HIGH fix — security review): the command body
    /// either legitimately ran and returned a typed `Err` (e.g.
    /// `documents_export_document` failing validation), OR Tauri rejected
    /// the call before the body ever ran at all — a missing/mistyped arg
    /// (`applications_delete` called without `keepDocuments`), an ACL
    /// denial, or an unregistered command name. Both cases serialize to the
    /// SAME shape (a bare string — `AppError::serialize` and Tauri's own
    /// ACL-rejection string are wire-indistinguishable), so this crate
    /// cannot tell them apart from the response alone — but BOTH must never
    /// be reported as `dispatched: true`; see [`Refusal::InvokeError`].
    CommandErr(Value),
}

/// Pure: `InvokeResponse` → [`InvokeOutcome`]. No `AppHandle`, no I/O — every
/// branch of [`invoke_command`]'s previous behaviour (folding
/// `InvokeResponse::Err` into a successful `Ok(Value)`) is what let a
/// Tauri-level rejection report `dispatched: true` for a command whose body
/// never ran; this split is what makes that mapping directly testable.
fn classify_response(response: InvokeResponse) -> InvokeOutcome {
    match response {
        InvokeResponse::Ok(InvokeResponseBody::Json(s)) => {
            InvokeOutcome::Success(serde_json::from_str(&s).unwrap_or(Value::Null))
        }
        // No command on the Read-effect rows returns a raw byte body today,
        // but degrade rather than drop it if one ever does.
        InvokeResponse::Ok(InvokeResponseBody::Raw(bytes)) => InvokeOutcome::Success(json!(bytes)),
        InvokeResponse::Err(InvokeError(v)) => InvokeOutcome::CommandErr(v),
    }
}

/// Drive one `Webview::on_message` round trip for `command`. `input` becomes
/// the invoke body verbatim — exactly what the renderer's own
/// `invoke(cmd, args)` sends (Tauri deserializes each top-level key into the
/// matching arg by name), so `--input '{"jobId":"..."}'` reaches the command
/// the same way a UI click would.
async fn invoke_command(app: &AppHandle, command: &str, input: Value) -> AppResult<InvokeOutcome> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::Config("main window unavailable".to_string()))?;
    let url = window
        .url()
        .map_err(|e| AppError::Message(format!("could not read the window url: {e}")))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    let request = InvokeRequest {
        cmd: command.to_string(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url,
        body: input.into(),
        headers: Default::default(),
        invoke_key: app.invoke_key().to_string(),
    };
    window.on_message(
        request,
        Box::new(move |_webview, _cmd, response, _callback, _error| {
            let _ = tx.send(response);
        }),
    );
    let response = rx
        .await
        .map_err(|_| AppError::Message("command dispatch never replied".to_string()))?;
    Ok(classify_response(response))
}

/// [`Refusal::InvokeError`]'s detail text — a bare JSON string (the common
/// case — both `AppError` and Tauri's own ACL-rejection serialize as one)
/// renders unquoted; anything else (rare — a future non-string command
/// error type) falls back to its JSON form rather than panicking.
fn invoke_error_detail(v: &Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

/// Reverses [`fence_named_fields_recursive`]'s wrapper on every INCOMING
/// `--input` value under a [`FENCE_FIELD_NAMES`] key, before ANY dispatched
/// command's real body ever sees it (security review round 4 — the
/// centralised fix: `commands::scrape::scrape_persist_job`'s own
/// `unfence_job_field` was a hand-added per-call-site strip, and every OTHER
/// freely-dispatchable WRITE command accepting one of these SAME field
/// names had none — three rounds of "add it at the call site" is what
/// produced that gap). A caller that reads a job through a fenced surface
/// (`scrape_list_postings`, `autopilot_list`, `ai_generations_list`, …) and
/// echoes a value straight back into a write would otherwise persist the
/// literal `<job_posting>…</job_posting>` markup into the user's own data —
/// this closes it for every CURRENT and FUTURE writer at the one chokepoint
/// every real dispatch already funnels through ([`dispatch_direct`], called
/// directly for `Read`/`Reversible` and at the tail of
/// [`dispatch_irreversible_confirmed`] for a confirmed `Irreversible`), not
/// one call site at a time. A no-op for the normal case — a clean value
/// that was never fenced — by [`crate::prompt_fence::strip_fence_wrapper`]'s
/// own contract (an exact prefix/suffix match, unchanged otherwise).
/// `commands::scrape::scrape_persist_job`'s own call-site strip is left in
/// place as defense-in-depth at the actual store-write boundary (that
/// command is also reachable from the renderer's normal `invoke()`, not
/// only through this dispatcher) rather than removed.
fn unfence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for field in FENCE_FIELD_NAMES {
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let stripped = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                    map.insert((*field).to_string(), json!(stripped));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            *s = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                        }
                    }
                }
            }
            for v in map.values_mut() {
                unfence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                unfence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}

/// Invoke a command for real: take this layer's own paging arguments off
/// `input` ([`take_list_page_args`]), strip any fence wrapper the caller
/// echoed back into it ([`unfence_named_fields_recursive`]), dispatch, then
/// fence any scraped text in the response ([`fence_scraped_fields`]), page it
/// ([`paginate_list_reply`]) and re-encode any raw byte field
/// ([`base64_byte_fields`]). Called directly for a `Read`/`Reversible` row,
/// and again at [`dispatch_irreversible_confirmed`]'s tail for a confirmed
/// `Irreversible` one — the ONE real-invocation chokepoint every dispatched
/// row funnels through, never a second copy of any of those steps.
///
/// The two response reshapes go LAST, after fencing, and in this order: the
/// paging trim's byte budget must measure the fenced bytes that will really
/// ship (fencing expands what it touches), and base64 must see the raw
/// `Vec<u8>` array rather than something a later step rewrote.
async fn dispatch_direct(
    app: &AppHandle,
    command: &str,
    mut input: Value,
) -> Result<Value, Refusal> {
    let page_args = take_list_page_args(command, &mut input)?;
    unfence_named_fields_recursive(&mut input);
    let outcome = invoke_command(app, command, input)
        .await
        .map_err(|e| Refusal::DispatchFailed(e.to_string()))?;
    let mut data = match outcome {
        InvokeOutcome::Success(v) => v,
        // The command body either legitimately ran and returned a typed
        // `Err`, or Tauri rejected the call before the body ever ran (bad
        // args, an ACL denial, an unregistered command) — see
        // `Refusal::InvokeError`'s own doc for why these two are
        // wire-indistinguishable and both refuse rather than dispatch.
        InvokeOutcome::CommandErr(v) => return Err(Refusal::InvokeError(invoke_error_detail(&v))),
    };
    fence_scraped_fields(&mut data);
    if let Some((offset, limit)) = page_args {
        data = paginate_list_reply(data, offset, limit);
    }
    base64_byte_fields(command, &mut data);
    Ok(data)
}

/// The whole decision [`dispatch_irreversible_confirmed`] makes, with the
/// `AppHandle` factored out — the impure wrapper below only resolves the
/// proof and hands the result here, so the ceremony's two refusal paths are
/// directly testable (the crate has no Tauri mock, so nothing taking a
/// concrete `&AppHandle` can be).
///
/// `run` is called at most ONCE and ONLY on an exact match — never before
/// the comparison, which is the property the tests mutation-check: a
/// version that ran first and compared after would dispatch an
/// irreversible command on a wrong `confirm`. It returns whatever the
/// caller's own run step produces (the async wrapper returns the UNAWAITED
/// future, so this core stays sync and pure).
fn confirm_and_run<T>(
    resolved: Option<String>,
    confirm: &str,
    run: impl FnOnce() -> T,
) -> Result<T, Refusal> {
    let expected = resolved.ok_or(Refusal::ProofUnavailable)?;
    if confirm != expected {
        return Err(Refusal::ConfirmationMismatch);
    }
    Ok(run())
}

/// Dispatch an `Irreversible` row whose `confirm` is already known to be
/// present (the caller — [`dispatch`] — only reaches here via
/// [`Dispatch::Confirmed`], produced by [`gate`]): resolve the expected
/// value FRESH via [`proof::resolve`] and only then run the real command.
/// A thin wrapper over [`confirm_and_run`] — the only thing that needs the
/// `AppHandle` is the resolve and the dispatch themselves.
async fn dispatch_irreversible_confirmed(
    app: &AppHandle,
    command: &str,
    input: Value,
    source: ProofSource,
    confirm: &str,
) -> Result<Value, Refusal> {
    let resolved = proof::resolve(app, source, &input).await;
    confirm_and_run(resolved, confirm, || dispatch_direct(app, command, input))?.await
}

/// What [`gate`] clears `dispatch` to do for one `(effect, confirm)` pair —
/// carries whatever the cleared branch needs, so nothing downstream
/// re-derives a fact `gate` already established. `Confirmed`'s `confirm` is
/// a plain `&str`, not an `Option` — reaching that variant at all is already
/// proof one was supplied, so there is nothing left to unwrap.
pub(super) enum Dispatch<'a> {
    /// `Read`/`Reversible` — invoke directly, no ceremony.
    Direct,
    /// `Irreversible`, `confirm` already known to be present. Carries the
    /// row's own [`ProofSource`] alongside it so `dispatch` never re-matches
    /// `entry.effect` a second time to recover it.
    Confirmed {
        source: ProofSource,
        confirm: &'a str,
    },
}

/// Pure gate: does `effect` permit `dispatch` to ATTEMPT a real command
/// invocation at all, given whether a `confirm` value was supplied — never
/// mind whether that attempt then succeeds. Replaces a former
/// boolean-returning `dispatchable`: a `bool` only told the caller "yes",
/// forcing `dispatch` to re-match `entry.effect` a second time to recover
/// the `ProofSource` AND `.expect()` a `confirm` this fn had already proved
/// `Some` — an `expect` on an externally reachable `agent.call` path, safe
/// only because of a separate call to this same gate rather than because
/// the type ruled out the `None` case. Returning [`Dispatch`] instead means
/// the confirmed branch carries its `&str` and `ProofSource` BY
/// CONSTRUCTION, so there is nothing left downstream to re-derive or
/// unwrap — a future refactor that changed this gate's logic could no
/// longer silently leave a stale, now-unsound `expect` behind it.
///
/// `dispatch` below calls this as its own FIRST decision (never a
/// parallel/shadow copy of the same logic), so `extension_bridge::test`'s
/// exhaustive walk over every real `POLICY` row
/// (`agent_call_gate_matches_every_policy_rows_declared_effect`) proves
/// something about THIS production routing, not a second implementation
/// that could silently drift from it. `pub(super)` — reachable from
/// `extension_bridge::test`, a sibling of this module, for exactly that
/// test; [`Dispatch`] shares that visibility for the same reason.
pub(super) fn gate(effect: Effect, confirm: Option<&str>) -> Result<Dispatch<'_>, Refusal> {
    match effect {
        Effect::NotExposed(reason) => Err(Refusal::NotExposed(reason)),
        Effect::Read | Effect::Reversible => Ok(Dispatch::Direct),
        Effect::Irreversible(source) => match confirm {
            Some(confirm) => Ok(Dispatch::Confirmed { source, confirm }),
            None => Err(Refusal::ConfirmationRequired(proof::hint(source))),
        },
    }
}

async fn dispatch(
    app: &AppHandle,
    namespace: &str,
    command: &str,
    input: Value,
    confirm: Option<&str>,
) -> Result<Value, Refusal> {
    let entry = find_policy(namespace, command).ok_or(Refusal::UnknownCommand)?;
    match gate(entry.effect, confirm)? {
        Dispatch::Direct => dispatch_direct(app, command, input).await,
        Dispatch::Confirmed { source, confirm } => {
            dispatch_irreversible_confirmed(app, command, input, source, confirm).await
        }
    }
}

/// Substitute a [`Refusal::ResultTooLarge`] reply for any `reply` the bridge
/// could not actually deliver — over [`super::MAX_FRAME_BYTES`], the cap both
/// ends of this socket configure (issue #1135; see that variant's own doc for
/// why an outgoing frame is otherwise unchecked and what the caller saw
/// instead). Pure, and returns the RECOMPUTED `dispatched` alongside the
/// reply so the observability span records what actually went on the wire
/// rather than what dispatch alone decided — measuring the built reply is the
/// only way to know, so this cannot live any earlier.
///
/// Note the asymmetry it deliberately preserves: `dispatched` on the wire
/// becomes `false` (no result was delivered, and every consumer — including
/// `agent_cli::exit_code_for_reply`'s exit-2 mapping — reads it that way),
/// while the refusal's own `detail` states plainly that the command RAN. The
/// substituted reply is always far smaller than the original, so this can
/// never itself exceed the cap.
fn enforce_frame_cap(
    req_id: &str,
    namespace: &str,
    command: &str,
    reply: String,
    dispatched: bool,
) -> (String, bool) {
    if reply.len() <= super::MAX_FRAME_BYTES {
        return (reply, dispatched);
    }
    let refused = call_result_reply(
        req_id,
        namespace,
        command,
        Err(Refusal::ResultTooLarge(reply.len())),
    );
    (refused, false)
}

/// Answer an authenticated, throttle-admitted, origin-checked `agent.call`.
/// Never panics — [`dispatch`] degrades to a [`Refusal`] on every failure
/// path (unknown command, wrong effect, or the dispatch itself erroring).
/// Logs the command identity + whether it dispatched (MEDIUM fix — security
/// review: every other privileged bridge path leaves an observability
/// record, this one didn't) — NEVER `input`/`confirm`/the response `data`,
/// every one of which can carry PII or a résumé/cover-letter body.
pub(super) async fn handle_agent_call(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let (namespace, command) = {
        let (ns, cmd) = payload_target(payload);
        (ns.to_string(), cmd.to_string())
    };
    let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));
    let confirm = payload.get("confirm").and_then(Value::as_str);

    let span = crate::observability::Span::begin(
        "agent_call",
        format!("namespace={namespace} command={command}"),
    );
    let outcome = dispatch(app, &namespace, &command, input, confirm).await;
    let dispatched = outcome.is_ok();
    let reply = call_result_reply(req_id, &namespace, &command, outcome);
    let (reply, dispatched) = enforce_frame_cap(req_id, &namespace, &command, reply, dispatched);
    span.end_with(&format!("dispatched={dispatched}"), dispatched);
    reply
}

#[cfg(test)]
mod tests;
