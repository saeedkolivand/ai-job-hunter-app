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
// The agent layer's own payload reshaping — the outbound fence/page/base64
// order and the inbound fence-strip mirror — lives in its own file under the
// R8 LOC cap; see `agent_call/reshape.rs`. Visible to the rest of
// `extension_bridge` for the three items `agent_cli::mcp` and `agent_read`
// read through it, the same shape `agent_read` uses for `found_jobs`.
pub(in crate::extension_bridge) mod reshape;
use reshape::{reshape_reply, take_list_page_args, unfence_named_fields_recursive};

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
    /// A caller-supplied `cursor` on one of
    /// [`reshape::PAGINATED_LIST_COMMANDS`] that isn't a plain non-negative
    /// integer offset. The detail is the FIXED
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
                 alternative if this command has one."
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

/// Longest caller-supplied `reqId`/`namespace`/`command` a REFUSAL reply
/// echoes back, in bytes. All three arrive off the wire bounded only by the
/// bridge's own INCOMING frame cap ([`super::MAX_FRAME_BYTES`], 8 MiB), so a
/// refusal that echoed them verbatim could itself exceed the very cap it
/// exists to enforce: a ~8.38 MB `command` made [`enforce_frame_cap`]'s
/// substitute measure 8,389,135 B against an 8,388,608 B ceiling (HIGH —
/// security review), and [`origin_refused_reply`]/[`throttled_reply`] echo
/// the same unbounded strings without ever passing through that check at all.
/// 256 is far above anything the real [`POLICY`] table can produce (its
/// longest command name is comfortably under a third of it) and far below
/// anything that could threaten a frame. That headroom is pinned against the
/// TABLE, not against a copy of the number here — see
/// `the_identifier_clamp_leaves_every_real_identifier_untouched`.
const REFUSAL_IDENT_CAP: usize = 256;

/// A [`REFUSAL_IDENT_CAP`]-bounded prefix of a caller-supplied identifier,
/// cut on a CHAR BOUNDARY — `&value[..256]` panics mid-codepoint, and this
/// crate is `panic = "abort"` in release, so a slice panic inside a frame
/// handler is a silent process death, not an error.
///
/// Applied on REFUSAL replies only, never on the success path: a real command
/// name is short, and a long one is already `unknown_command`, so clamping
/// there could only make a legitimate reply lie about which command produced
/// it.
fn clamp_ident(value: &str) -> &str {
    if value.len() <= REFUSAL_IDENT_CAP {
        return value;
    }
    let mut end = REFUSAL_IDENT_CAP;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// `detail` on [`refusal_reply`]'s last-resort envelope. A FIXED string, so
/// that envelope's size cannot be influenced by anything the caller sent.
const REFUSAL_UNDELIVERABLE_DETAIL: &str =
    "this call's refusal did not fit the bridge's frame cap, so its identifiers and detail \
     were dropped in order to deliver any reply at all";

/// The ONE builder for every refusal on this surface — the success path keeps
/// calling [`call_result_reply`] directly. Two properties, neither of which
/// the raw builder can offer:
///
/// 1. **Bounded material only.** The three echoed identifiers go through
///    [`clamp_ident`]; every `detail` is either a fixed literal, a `'static`
///    reason off a [`POLICY`] row, a `proof::hint` built from `'static` field
///    names, or (for [`Refusal::InvokeError`]) a string already capped by
///    `crate::prompt_fence::JOB_CAP`. So the whole reply is bounded by
///    construction, not by hoping the inputs were small.
/// 2. **Measured, not assumed.** Property 1 is an argument about today's
///    variants; this re-measures the built reply anyway and degrades to a
///    minimal envelope (empty identifiers, fixed detail, identical `type` and
///    key set) if it somehow still does not fit. A refusal that blew the cap
///    would reproduce the exact failure it reports — the caller would get the
///    content-free `connection_lost` that issue #1135 exists to eliminate.
fn refusal_reply(req_id: &str, namespace: &str, command: &str, refusal: Refusal) -> String {
    let sentinel = refusal.sentinel();
    let reply = call_result_reply(
        clamp_ident(req_id),
        clamp_ident(namespace),
        clamp_ident(command),
        Err(refusal),
    );
    if reply.len() <= super::MAX_FRAME_BYTES {
        return reply;
    }
    json!({
        "type": super::msg::AGENT_CALL_RESULT,
        "reqId": "",
        "payload": {
            "dispatched": false,
            "namespace": "",
            "command": "",
            "error": sentinel,
            "detail": REFUSAL_UNDELIVERABLE_DETAIL,
        },
    })
    .to_string()
}

/// Reply for an `agent.call` arriving over a connection whose handshake
/// `Origin` wasn't `auth::AGENT_CLI_ORIGIN` — mirrors
/// `agent_read::origin_refused_reply` exactly, one wire type over. Built
/// through [`refusal_reply`]: this path never reaches [`enforce_frame_cap`]
/// (`mod.rs` writes what it returns straight to the socket), so the clamp is
/// the ONLY thing bounding what it echoes.
pub(super) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    refusal_reply(req_id, namespace, command, Refusal::OriginRefused)
}

/// Same never-reaches-[`enforce_frame_cap`] path as [`origin_refused_reply`],
/// and bounded the same way.
pub(super) fn throttled_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    refusal_reply(req_id, namespace, command, Refusal::RateLimited)
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
/// doc comment above).
///
/// `ApplicationAnswer.question` — the one THIRD-PARTY item that list used to
/// flag as a plausible future candidate (a scraped ATS form's own question
/// label, same reasoning as `title`/`jobDescription`) — IS fenced now, but
/// by SHAPE and never by name: see [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for
/// why a flat `question` entry HERE would have silently re-fenced
/// `InterviewQuestion.question`, which shares the exact wire key on the same
/// command's response and is this app's own AI output.
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

/// `ai_generations::ApplicationAnswer`'s own always-present sibling key —
/// used to detect an `ApplicationAnswer`-shaped object (`{id, question,
/// answer}`, reachable through `applications_list`/`applications_get`/
/// `ai_generations_list`) so its [`APPLICATION_ANSWER_QUESTION_FIELD`] — a
/// THIRD-PARTY ATS form's own question label, captured from the page by
/// `extension_bridge::answers_save` — is fenced by SHAPE rather than by
/// name.
///
/// Deliberately NOT a [`FENCE_FIELD_NAMES`] entry: a flat name entry would
/// ALSO re-fence `ai_generations::InterviewQuestion.question` (`{id,
/// question, why, audience}`), which serializes under the EXACT same wire
/// key, rides the SAME command's response, and is this app's own AI
/// coaching output — the one thing that const's own doc says it excludes on
/// purpose (ADR-038 §5's separate axis). `answer` is the discriminator: an
/// `ApplicationAnswer` always carries one, an `InterviewQuestion` never
/// does.
///
/// Note `extension_bridge::answers_suggest::answers_suggest_reply` builds a
/// sibling `{question, answer}` object too, but it is a BRIDGE frame, not a
/// dispatched command response, so it never reaches this walk; were that
/// shape ever to move onto this surface it would simply be fenced the same
/// way — the safe direction.
const APPLICATION_ANSWER_ANCHOR_FIELDS: [&str; 1] = ["answer"];

/// The single key [`APPLICATION_ANSWER_ANCHOR_FIELDS`] guards, named once so
/// [`fence_named_fields_recursive`] and [`unfence_named_fields_recursive`]
/// can never disagree about which field the shape rule covers.
const APPLICATION_ANSWER_QUESTION_FIELD: &str = "question";

/// `jobs::JobRecord`'s own always-present, distinctively-named fields
/// (`kind`, `progress`, `max_retries` → `maxRetries` under that struct's
/// `#[serde(rename_all = "camelCase")]`) — used to detect a
/// `JobRecord`-shaped object (`jobs_get`, `jobs_list`) so
/// [`JOB_RECORD_RESULT_FIELD`] can be EXEMPTED from the name-keyed walk.
///
/// A completed job's `result` is the app's OWN output — a generated draft or
/// a model answer under `{"done": true, "text": …}`
/// (`commands::ai_provider::stream`, `commands::resume_pipeline`) — while
/// `text` is on [`FENCE_FIELD_NAMES`] for `documents::DocumentRecord.text`,
/// so before this exemption every generation read back through `jobs_get`
/// reached the caller wrapped as a scraped posting. Verified no other struct
/// on this dispatch surface serializes all three anchors together
/// (`maxRetries` has exactly one producer in the crate).
///
/// The exemption is WHOLESALE for the NAME-keyed walk and audited, not
/// shape-inspected per value: no [`FENCE_FIELD_NAMES`] entry fires anywhere
/// under `result`, so a job kind that starts putting THIRD-PARTY text there
/// must fence it itself. The warning that says so lives on
/// `commands::jobs::job_complete` — the single mutator every completion
/// funnels through — rather than on each producer.
///
/// The scrape-diagnostics shapes are carved back out, because auditing the
/// producer list turned up a completion that already carried third-party
/// text: [`SCRAPE_SUMMARY_ANCHOR_FIELDS`] and [`BOARD_HEALTH_ANCHOR_FIELDS`]
/// fence a `BoardScrapeSummary`'s board-written strings wherever they sit
/// inside `result`. Those are shape rules with enumerated field sets, not a
/// reopening of the name walk — see [`fence_scrape_summaries_recursive`] for
/// why the distinction is load-bearing.
const JOB_RECORD_ANCHOR_FIELDS: [&str; 3] = ["kind", "progress", "maxRetries"];

/// The one `JobRecord` field [`JOB_RECORD_ANCHOR_FIELDS`] exempts. Every
/// other field still recurses — `payload` included, since a dispatch payload
/// CAN carry a scraped posting.
const JOB_RECORD_RESULT_FIELD: &str = "result";

/// `scraping::engine::BoardScrapeSummary`'s own always-present field pair
/// (`board`, `count` — both non-`Option`, and single words that its
/// `#[serde(rename_all = "camelCase")]` leaves unchanged) — used to detect a
/// summary-shaped object so [`SCRAPE_SUMMARY_UNTRUSTED_FIELDS`] can be fenced
/// by SHAPE.
///
/// Shape and never a [`FENCE_FIELD_NAMES`] row, for the same reason
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] is: `error` is one of the most
/// generic keys on this whole surface — `jobs::JobRecord.error` itself, plus
/// every refusal envelope — and a flat name entry would wrap this app's own
/// already-sanitized error strings as though a job board had written them.
///
/// Verified distinctive on this dispatch surface: `board` occurs WITHOUT a
/// sibling `count` on `board_health::BoardHealthEntry` (`{board, health}`)
/// and on a cluster member (`{key, board?, url}`), and `count` occurs
/// without a `board` on the `scrape_*` completion envelopes themselves
/// (`{count, boards}` / `{count}`) — no other struct in the crate
/// serializes both together.
const SCRAPE_SUMMARY_ANCHOR_FIELDS: [&str; 2] = ["board", "count"];

/// The board/provider-derived strings a [`SCRAPE_SUMMARY_ANCHOR_FIELDS`]-
/// detected object carries. Each is written by the REMOTE side of a scrape,
/// not by this app: `error` is a board's own failure text (an aggregator
/// provider prefixes its own name onto whatever the upstream API returned),
/// `skipped` its refusal reason, `truncated` a mid-run page failure. Its
/// siblings are not here on purpose — `board`/`count` are the anchors and
/// `notes` is a fixed engine vocabulary. `health` is not a string at all;
/// its own board-written carrier is covered by
/// [`BOARD_HEALTH_ANCHOR_FIELDS`] below.
///
/// These reach an agent through a completed `scrape_boards` job
/// (`jobs_get`/`jobs_list`, where [`JOB_RECORD_RESULT_FIELD`] otherwise
/// exempts the whole subtree) and through `Autopilot.last_run_summaries`
/// (`autopilot_list`/`autopilot_get`), so the rule is applied to the shape
/// wherever it appears rather than to either route.
const SCRAPE_SUMMARY_UNTRUSTED_FIELDS: [&str; 3] = ["error", "skipped", "truncated"];

/// `scraping::board_health::BoardHealth`'s own always-present field pair
/// (`status`, `consecutive_failures` → `consecutiveFailures`) — the SECOND
/// shape carrying board-written text in the same payload, because
/// `board_health::fold` copies `BoardScrapeSummary.error` FORWARD into
/// `BoardHealth.last_error`. That copy runs through `clean_error`, which
/// redacts paths/hosts and caps the length — a redactor, not a controlled
/// vocabulary — so the board's own prose survives it intact and is exactly
/// as untrusted as the `error` it came from. Fencing one and not the other
/// would leave the same sentence reachable one level deeper, under
/// `summary.health.lastError`, and standalone on a `BoardHealthEntry.health`.
///
/// `consecutiveFailures` is the distinctive half: it is the only serialized
/// field of that name in the crate (verified), so no other struct on this
/// surface can be mistaken for this shape.
const BOARD_HEALTH_ANCHOR_FIELDS: [&str; 2] = ["status", "consecutiveFailures"];

/// The one board-written string on a [`BOARD_HEALTH_ANCHOR_FIELDS`]-detected
/// object. Its siblings are counters, epoch-ms timestamps, a derived status
/// enum and this app's own scrape `job_id` — none of them third-party text.
const BOARD_HEALTH_UNTRUSTED_FIELDS: [&str; 1] = ["lastError"];

/// True when `map` is an `ai_generations::ApplicationAnswer`-shaped object:
/// a STRING [`APPLICATION_ANSWER_QUESTION_FIELD`] plus every
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] key. Shared by the fence and the
/// unfence walk so the two can never disagree about the shape.
fn is_application_answer_shaped(map: &serde_json::Map<String, Value>) -> bool {
    map.get(APPLICATION_ANSWER_QUESTION_FIELD)
        .is_some_and(Value::is_string)
        && APPLICATION_ANSWER_ANCHOR_FIELDS
            .iter()
            .all(|f| map.contains_key(*f))
}

/// Fence the board-written strings on `map` when its keys match either
/// scrape-diagnostics shape — [`SCRAPE_SUMMARY_ANCHOR_FIELDS`] →
/// [`SCRAPE_SUMMARY_UNTRUSTED_FIELDS`], [`BOARD_HEALTH_ANCHOR_FIELDS`] →
/// [`BOARD_HEALTH_UNTRUSTED_FIELDS`] — and nothing at all on any other
/// object. The two shapes are checked independently rather than nested: a
/// `BoardHealth` also reaches this surface standalone, on a
/// `BoardHealthEntry`, not only under a summary's `health`.
///
/// Shared by [`fence_named_fields_recursive`] (diagnostics anywhere OUTSIDE
/// a job result) and [`fence_scrape_summaries_recursive`] (the copies INSIDE
/// the otherwise-exempt one), so the two walks can never disagree about
/// either shape or either field set.
///
/// Fencing happens on this READ path rather than at the producer
/// (`commands::scrape::scrape_boards`, before `job_complete`) on purpose:
/// the very same strings are what the renderer's per-board chip strip
/// displays — `BoardSummaryChips` matches `skipped` against a controlled
/// vocabulary to pick a localized label, and renders `error`, `truncated`
/// and `health.lastError` as chip detail — reached both by the
/// `job.completed` event and, on remount, by the watchdog's own `jobs_get`.
/// A fence baked into the stored result would put `<job_posting>` markup on
/// screen and knock `skipped` out of every arm of that match; stripping it
/// back off in the renderer would mean a second, hand-maintained copy of
/// these field lists in TypeScript, on a path where a miss is visible to the
/// user.
fn fence_board_derived_strings(map: &mut serde_json::Map<String, Value>) {
    for (anchors, fields) in [
        (
            SCRAPE_SUMMARY_ANCHOR_FIELDS.as_slice(),
            SCRAPE_SUMMARY_UNTRUSTED_FIELDS.as_slice(),
        ),
        (
            BOARD_HEALTH_ANCHOR_FIELDS.as_slice(),
            BOARD_HEALTH_UNTRUSTED_FIELDS.as_slice(),
        ),
    ] {
        if !anchors.iter().all(|f| map.contains_key(*f)) {
            continue;
        }
        for field in fields {
            if let Some(s) = map.get(*field).and_then(Value::as_str) {
                let fenced =
                    crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
                map.insert((*field).to_string(), json!(fenced));
            }
        }
    }
}

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
///
/// Some rules are keyed on an object's SHAPE rather than a field name,
/// because a name alone cannot tell two carriers apart:
/// [`APPLICATION_ANSWER_ANCHOR_FIELDS`] fences a scraped ATS `question`
/// without touching `InterviewQuestion.question`;
/// [`JOB_RECORD_ANCHOR_FIELDS`] exempts a job's own `result` so a generation
/// read back through `jobs_get` is not labelled as scraped posting text; and
/// [`SCRAPE_SUMMARY_ANCHOR_FIELDS`]/[`BOARD_HEALTH_ANCHOR_FIELDS`] fence the
/// board-WRITTEN strings on a `BoardScrapeSummary`/`BoardHealth` without
/// touching this app's own same-named `error` strings — including inside
/// that exempt `result`, which is where a completed `scrape_boards` job puts
/// them.
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
///
/// Then the shape rules: on an [`APPLICATION_ANSWER_ANCHOR_FIELDS`]-
/// detected object the [`APPLICATION_ANSWER_QUESTION_FIELD`] string is
/// fenced (a scraped ATS question label whose wire key is shared with this
/// app's own `InterviewQuestion.question`), on a scrape-diagnostics object
/// [`fence_board_derived_strings`] fences the board-written keys, and on a
/// [`JOB_RECORD_ANCHOR_FIELDS`]-detected object the recursion hands
/// [`JOB_RECORD_RESULT_FIELD`] to [`fence_scrape_summaries_recursive`]
/// instead of walking it (a job's own output, not scraped text — except for
/// the diagnostics a scrape completes with).
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
            let job_posting_shaped = JOB_POSTING_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            if job_posting_shaped {
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
            // Shape-guarded, never a name entry — see
            // [`APPLICATION_ANSWER_ANCHOR_FIELDS`] for why putting
            // `question` on [`FENCE_FIELD_NAMES`] would have re-fenced
            // `InterviewQuestion.question`. Skipped on a `JobPosting`-shaped
            // object: the catch-all above already fenced every unclassified
            // string there, and fencing twice would leave a wrapper behind
            // after [`unfence_named_fields_recursive`]'s single strip.
            if !job_posting_shaped && is_application_answer_shaped(map) {
                if let Some(question) = map
                    .get(APPLICATION_ANSWER_QUESTION_FIELD)
                    .and_then(Value::as_str)
                {
                    let fenced = crate::prompt_fence::fenced(
                        "job_posting",
                        question,
                        crate::prompt_fence::JOB_CAP,
                    );
                    map.insert(APPLICATION_ANSWER_QUESTION_FIELD.to_string(), json!(fenced));
                }
            }
            // The scrape-diagnostics shape rules, under the same
            // `!job_posting_shaped` guard and for the same reason: on a
            // `JobPosting`-shaped object the
            // `extra` catch-all above already fenced every unclassified
            // string, and `fenced` does NOT guard against double-wrapping
            // (nor does `fence_all_string_leaves`), so a second pass would
            // leave a wrapper behind after
            // [`unfence_named_fields_recursive`]'s single strip. Reached by
            // `Autopilot.last_run_summaries`; the copies inside a
            // `JobRecord`'s exempt `result` are handled below.
            if !job_posting_shaped {
                fence_board_derived_strings(map);
            }
            // A `JobRecord`'s own `result` is the app's OWN output, not
            // scraped text — see [`JOB_RECORD_ANCHOR_FIELDS`]. The exemption
            // is on the RECURSION only: every other field of this object,
            // and every other object in the tree, walks as before.
            let job_record_shaped = JOB_RECORD_ANCHOR_FIELDS
                .iter()
                .all(|f| map.contains_key(*f));
            for (key, v) in map.iter_mut() {
                if job_record_shaped && key.as_str() == JOB_RECORD_RESULT_FIELD {
                    // The exemption is wholesale for the NAME-keyed walk, and
                    // stays that way — but `scrape_boards` completes with
                    // `BoardScrapeSummary` rows, so a diagnostics shape does
                    // carry third-party text in here. Fence only those
                    // enumerated keys and nothing else in the subtree; see
                    // [`SCRAPE_SUMMARY_ANCHOR_FIELDS`].
                    fence_scrape_summaries_recursive(v);
                    continue;
                }
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

/// Walk `value` applying ONLY [`fence_board_derived_strings`] — the single
/// carve-out inside a `JobRecord`'s otherwise-exempt
/// [`JOB_RECORD_RESULT_FIELD`]. Deliberately NOT
/// [`fence_named_fields_recursive`]: running the name-keyed walk in here
/// would re-open the exact defect the exemption exists to close (a
/// generation's `{"done": true, "text": …}` labelled as a scraped posting).
/// A scrape summary and its board health are fenced; everything else in the
/// subtree is left exactly as the producer wrote it.
fn fence_scrape_summaries_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            fence_board_derived_strings(map);
            for v in map.values_mut() {
                fence_scrape_summaries_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_scrape_summaries_recursive(item);
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

/// Invoke a command for real: take this layer's own paging arguments off
/// `input` ([`take_list_page_args`]), strip any fence wrapper the caller
/// echoed back into it ([`unfence_named_fields_recursive`]), dispatch, then
/// fence any scraped text in the response ([`fence_scraped_fields`]), page it
/// ([`reshape::paginate_list_reply`]) and re-encode any raw byte field
/// ([`reshape::base64_byte_fields`]). Called directly for a `Read`/`Reversible`
/// row, and again at [`dispatch_irreversible_confirmed`]'s tail for a confirmed
/// `Irreversible` one — the ONE real-invocation chokepoint every dispatched
/// row funnels through, never a second copy of any of those steps.
///
/// The response side of that is [`reshape_reply`], which owns the ORDER the
/// three steps run in — extracted so the order is one pure, directly
/// testable fn rather than three statements whose sequence nothing pins.
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
    let data = match outcome {
        InvokeOutcome::Success(v) => v,
        // The command body either legitimately ran and returned a typed
        // `Err`, or Tauri rejected the call before the body ever ran (bad
        // args, an ACL denial, an unregistered command) — see
        // `Refusal::InvokeError`'s own doc for why these two are
        // wire-indistinguishable and both refuse rather than dispatch.
        InvokeOutcome::CommandErr(v) => return Err(Refusal::InvokeError(invoke_error_detail(&v))),
    };
    Ok(reshape_reply(command, data, page_args))
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
/// while the refusal's own `detail` states plainly that the command RAN.
///
/// The substitute is NOT "always far smaller than the original" — that was
/// the false absolute this doc used to claim (HIGH, security review). Its
/// `data` is gone, but it still echoes `reqId`/`namespace`/`command`, and all
/// three are caller-supplied and bounded only by the 8 MiB INCOMING frame: a
/// ~8.38 MB `command` produced a substitute of 8,389,135 B against an
/// 8,388,608 B ceiling, i.e. a refusal that reproduced the failure it
/// reports. What holds instead is a BOUNDED argument, and it lives in
/// [`refusal_reply`]: the identifiers are clamped to [`REFUSAL_IDENT_CAP`],
/// every `detail` is bounded by construction, and the built reply is
/// re-measured with a minimal-envelope fallback. Hence this fn no longer
/// builds the substitute itself.
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
    let refused = refusal_reply(
        req_id,
        namespace,
        command,
        Refusal::ResultTooLarge(reply.len()),
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
    // Success builds from the raw builder (identifiers verbatim); EVERY
    // refusal goes through `refusal_reply`, which is where the identifier
    // clamp lives — a caller-supplied `command` of any length arrives here as
    // `Refusal::UnknownCommand` long before it could reach the frame cap.
    let reply = match outcome {
        Ok(data) => call_result_reply(req_id, &namespace, &command, Ok(data)),
        Err(refusal) => refusal_reply(req_id, &namespace, &command, refusal),
    };
    let (reply, dispatched) = enforce_frame_cap(req_id, &namespace, &command, reply, dispatched);
    span.end_with(&format!("dispatched={dispatched}"), dispatched);
    reply
}

#[cfg(test)]
mod tests;
