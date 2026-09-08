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
// Dispatch-time input-key validation against the generated
// `agent_cli::catalogue` (issues #1163, #1158, #1160) — its own file under
// the same R8 LOC-cap reasoning as `proof`/`reshape` above.
mod validate;
// The pure `gate`/`plan` ordering decision — same R8 LOC-cap reasoning again. Re-exported here so
// every existing `agent_call::gate` / `super::gate` call site is unchanged.
mod dispatch_plan;
pub(super) use dispatch_plan::{plan, Dispatch};
// `gate` itself has no non-test caller left in THIS module (production code reaches it only
// through `plan`, inside `dispatch_plan.rs`) — every other caller is a `#[cfg(test)]` module
// (`agent_call::tests`, `extension_bridge::test`), so this re-export is test-only too.
#[cfg(test)]
pub(super) use dispatch_plan::gate;

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

/// The real namespace for `command`, when EXACTLY ONE [`POLICY`] row's own bare command name
/// matches it — never a fuzzy match on a mistyped COMMAND name (issue #1163's `unknown_command`
/// naming request is scoped to "the bare command name matches exactly one row": this is an EXACT
/// string match on the trailing segment, the same equality [`find_policy`] itself uses, not a
/// distance/prefix heuristic). `None` when zero rows match (the command name itself is wrong, not
/// just its namespace) or — defensively, since `generate_handler!` requires globally-unique
/// command names, so this can't happen for a real row — more than one does; guessing between two
/// would be exactly the "typo to a destructive neighbour" path this surface never takes.
/// `pub(super)` — the MCP server's own LOCAL `unknown_command` refusal
/// ([`super::agent_cli::mcp::local_call_refusal`]) needs the identical suggestion, never a second
/// hand-typed scan of [`POLICY`].
pub(super) fn namespace_suggestion(command: &str) -> Option<&'static str> {
    let mut matches = POLICY
        .iter()
        .filter(|entry| split_path(entry.path).1 == command)
        .map(|entry| split_path(entry.path).0);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// [`Refusal::UnknownCommand`]'s own detail text — built from [`namespace_suggestion`]'s output,
/// `pub(super)` so [`super::agent_cli::mcp::local_call_refusal`] can build the IDENTICAL wording
/// for its own local (never-dispatched, no round trip) refusal rather than a second hand-typed
/// copy that could drift.
pub(super) fn unknown_command_detail(suggestion: Option<&str>) -> String {
    match suggestion {
        Some(namespace) => format!(
            "no policy row matches this <namespace>:<command> — this command name IS real, but \
             registered under namespace `{namespace}`; run `agent schema` or the MCP `commands` \
             tool to enumerate targets, or see policy.rs for the full table"
        ),
        None => "no policy row matches this <namespace>:<command> — run `agent schema` or the \
                  MCP `commands` tool to enumerate targets, or see policy.rs for the full table"
            .to_string(),
    }
}

/// Re-exports of [`proof::proof_field`]/[`proof::proof_kind`] for `commands`' `proofField`/
/// `proofKind` rows (issues #1160, #1160 round 2), without widening `proof`'s own module privacy.
pub(super) fn proof_field_for(source: ProofSource) -> Option<String> {
    proof::proof_field(source)
}

pub(super) fn proof_kind_for(source: ProofSource) -> &'static str {
    proof::proof_kind(source)
}

/// Re-export of [`validate::check_input`] + [`validate::check_no_empty_required_wrapper`] for
/// MCP's `local_call_refusal` (A1-r1-SEC-1 HIGH, widened for A1-r1-AC-1/SEC-2-round-2 MEDIUM):
/// `local_call_refusal` used to refuse only `unknown_command`/`not_exposed`/`wrong_tool` locally
/// and forward every other body straight to the PEER app process for catalogue validation — a
/// SEPARATE, possibly OLDER process (e.g. an updater-staged newer exe still paired with it), so
/// relying on its gate left a mis-keyed `call-*` body dispatching silently on an older running app
/// even though this server's own `initialize` instructions promise `invalid_input` is refused
/// before dispatch. Mirroring only `check_input` and not its sibling left the OTHER half of that
/// same gap open: an empty required wrapper (`{"req":{}}`, issue #1158's headline symptom) still
/// depended on the peer app to refuse it. Both checks run here, in [`dispatch_plan::plan`]'s own
/// order, so the local mirror matches the app-side gate exactly rather than half of it. Returns
/// the detail string (never the full [`Refusal`], to keep `validate`'s enum-construction private
/// to this module).
pub(super) fn invalid_input_detail(command: &str, effect: Effect, input: &Value) -> Option<String> {
    fn detail_of(result: Result<(), Refusal>) -> Option<String> {
        match result {
            Ok(()) => None,
            Err(Refusal::InvalidInput(detail)) => Some(detail),
            Err(_) => None, // both checked fns' only Err variant is InvalidInput
        }
    }
    detail_of(validate::check_input(command, input)).or_else(|| {
        detail_of(validate::check_no_empty_required_wrapper(
            command, effect, input,
        ))
    })
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
    /// No policy row matches this `(namespace, command)` pair at all. Carries
    /// [`namespace_suggestion`]'s own output — the real namespace, when the bare command name
    /// itself is real and unambiguous — so the refusal can name it without a second lookup.
    UnknownCommand(Option<&'static str>),
    /// The caller's `input` failed the generated catalogue's declared contract (issues #1163,
    /// #1158, #1160): an unknown top-level or nested key, or a missing required top-level key.
    /// Checked in [`plan`], AFTER [`Refusal::NotExposed`] but BEFORE [`gate`]'s confirm ceremony
    /// (see [`plan`]'s own doc for the ordering rationale). A command absent from the generated
    /// catalogue is unchecked (see `validate`'s own doc) — never constructed for one.
    InvalidInput(String),
    /// [`Effect::NotExposed`] — refused in [`plan`] BEFORE catalogue validation runs (CLI review
    /// round 2 — MEDIUM: 14/23 `NotExposed` rows carry declared args, so a bad key used to surface
    /// `InvalidInput` — a lesser cause masking the definitive one). Carries the row's own reason.
    NotExposed(&'static str),
    /// `agent.call` arrived over a connection whose handshake `Origin`
    /// wasn't the CLI's — same class as `msg::AGENT_QUERY`'s origin gate.
    OriginRefused,
    /// The shared throttle bucket is empty; carries `retryAfterMs` off it (issue #1155).
    RateLimited { retry_after_ms: u64 },
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
    /// A `confirm` value that did not match. `moved: true` (issue #1162) is the narrower case:
    /// `presented` matches a value THIS process itself disclosed the shape of via an earlier
    /// `confirmation_required` refusal for the SAME command, but the live proof has since moved
    /// (ordinary background AI activity against a spend-based proof) and the remembered
    /// snapshot's grace window has already closed — see `proof::accepted`/`SnapshotOutcome`. Same
    /// sentinel either way (never a stronger signal to probe with); the detail differs so a
    /// caller that did everything right is told to re-read rather than "you guessed wrong". The
    /// detail is otherwise a FIXED string, never the hint and never the expected value — a
    /// mismatch must not leak anything a caller couldn't already have gotten from the
    /// `ConfirmationRequired` refusal alone.
    ConfirmationMismatch { moved: bool },
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
/// [`Refusal::InvalidInput`]'s sentinel. `pub(super)` (A1-r1-SEC-1 HIGH) — the MCP server's
/// `local_call_refusal` now runs [`invalid_input_detail`] locally too, refusing with this SAME
/// sentinel rather than trusting the peer app's own dispatch-time check.
pub(super) const ERR_INVALID_INPUT: &str = "invalid_input";
/// `pub(super)` — the MCP server refuses a `NotExposed` row LOCALLY with this
/// SAME sentinel (so the token-row fix does not depend on the peer app's build;
/// see `agent_cli::mcp::local_call_refusal`), never a second hand-typed copy.
pub(super) const ERR_NOT_EXPOSED: &str = "not_exposed";
const ERR_CLI_ONLY: &str = "cli_only";
pub(super) const ERR_RATE_LIMITED: &str = "rate_limited"; // reused by agent_read, issue #1155
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
            Refusal::UnknownCommand(_) => ERR_UNKNOWN_COMMAND,
            Refusal::InvalidInput(_) => ERR_INVALID_INPUT,
            Refusal::NotExposed(_) => ERR_NOT_EXPOSED,
            Refusal::OriginRefused => ERR_CLI_ONLY,
            Refusal::RateLimited { .. } => ERR_RATE_LIMITED,
            Refusal::DispatchFailed(_) => ERR_DISPATCH_FAILED,
            Refusal::InvokeError(_) => ERR_INVOKE_ERROR,
            Refusal::ConfirmationRequired(_) => ERR_CONFIRMATION_REQUIRED,
            Refusal::ConfirmationMismatch { .. } => ERR_CONFIRMATION_MISMATCH,
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
    /// [`Refusal::InvokeError`]'s `detail` is DELIBERATELY UNLABELLED (issue #1157, owner
    /// decision -- flagged for `tauri-security-reviewer`): most of the time this is the app's OWN
    /// Tauri argument-validation sentence (a missing/mistyped arg, an ACL denial, an unregistered
    /// command) -- the single most actionable line an agent gets anywhere on this surface, and
    /// wrapping it as `<job_posting>` markup made a first-party diagnostic read as though a job
    /// board had written it. Capped at [`crate::prompt_fence::JOB_CAP`] chars and run through
    /// [`crate::prompt_fence::neutralize_transcript_boundaries`] (security review round A3-r1,
    /// AC-2/SEC-3 HIGH: unlabelling must not also drop the boundary defence) -- the SAME two
    /// causes are wire-indistinguishable (`agent_call.rs`'s own module doc), and the command-error
    /// cause CAN embed third-party/remote text (a scrape/HTTP/provider failure echoing part of a
    /// caller-chosen host's own response, e.g. `ai_pull_model`'s Ollama body or a provider's raw
    /// error message) -- so this string still gets the same forged-`</job_posting>`-tag defence
    /// every other untrusted string on this surface gets, just without the `<job_posting>` label
    /// and cap-truncation-into-a-wrapper that made a first-party sentence unreadable.
    fn detail(&self) -> String {
        match self {
            Refusal::UnknownCommand(suggestion) => unknown_command_detail(*suggestion),
            Refusal::InvalidInput(detail) => detail.clone(),
            Refusal::NotExposed(reason) => format!("not exposed to any CLI tier: {reason}"),
            Refusal::OriginRefused => CLI_ONLY_MESSAGE.to_string(),
            Refusal::RateLimited { .. } => super::agent_read::THROTTLED_MESSAGE.to_string(),
            Refusal::DispatchFailed(detail) => detail.clone(),
            Refusal::InvokeError(detail) => {
                // Capped and defused, never fenced/labelled -- see this variant's own `detail()`
                // doc above.
                let capped: String = detail.chars().take(crate::prompt_fence::JOB_CAP).collect();
                let capped = crate::prompt_fence::neutralize_transcript_boundaries(&capped);
                format!(
                    "the command either ran and returned an error, or Tauri rejected the call \
                     before the body ran (missing/invalid args, an ACL denial, or an unregistered \
                     command) — these are wire-indistinguishable; underlying value: {capped}"
                )
            }
            Refusal::ConfirmationRequired(hint) => hint.clone(),
            Refusal::ConfirmationMismatch { moved: false } => {
                "the confirm value did not match — it is never disclosed by this refusal; \
                 re-read the source named in a fresh confirmation_required refusal for this \
                 same command"
                    .to_string()
            }
            // Issue #1162 -- `presented` matched a value THIS command legitimately disclosed
            // the shape of earlier (a `confirmation_required` refusal), but the live proof has
            // moved since (background AI activity against a spend-based proof) and that
            // snapshot's grace window has closed. Same sentinel as an ordinary wrong guess
            // (never a stronger signal to probe with); the detail differs so a caller that did
            // everything right is told to re-read rather than "you guessed wrong" -- see
            // `proof::accepted`/`SnapshotOutcome::Expired`.
            Refusal::ConfirmationMismatch { moved: true } => {
                "the proof moved since it was disclosed (background AI activity) -- re-read it \
                 from a fresh confirmation_required refusal"
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
        Err(refusal) => {
            let mut payload = json!({
                "dispatched": false,
                "namespace": namespace,
                "command": command,
                "error": refusal.sentinel(),
                "detail": refusal.detail(),
            });
            // `retryAfterMs` present ONLY on the throttle refusal (issue
            // #1155) — matching `agent_read::sentinel_refusal_reply`'s
            // `extra` merge, which likewise omits the key entirely for
            // every other refusal. A client keying on presence
            // (`if ('retryAfterMs' in p) wait(...)`) must see the SAME
            // presence/absence split on both tiers; an always-present
            // `null` would make it wait 0 ms on a non-throttle refusal.
            if let Refusal::RateLimited { retry_after_ms } = &refusal {
                payload["retryAfterMs"] = json!(retry_after_ms);
            }
            payload
        }
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
/// it. `pub(super)` (#1151) — reused by `agent_read`'s own refusal builder.
pub(super) fn clamp_ident(value: &str) -> &str {
    if value.len() <= REFUSAL_IDENT_CAP {
        return value;
    }
    let mut end = REFUSAL_IDENT_CAP;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// `detail` on [`refusal_reply`]'s last-resort envelope. A FIXED string, so that envelope's size
/// cannot be influenced by anything the caller sent. `pub(super)` (#1151) — reused by `agent_read`.
pub(super) const REFUSAL_UNDELIVERABLE_DETAIL: &str =
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

/// Same never-reaches-[`enforce_frame_cap`] path as [`origin_refused_reply`], bounded the same
/// way. `retry_after_ms` comes from the one caller, `mod.rs` (issue #1155).
pub(super) fn throttled_reply(req_id: &str, payload: &Value, retry_after_ms: u64) -> String {
    let (namespace, command) = payload_target(payload);
    let refusal = Refusal::RateLimited { retry_after_ms };
    refusal_reply(req_id, namespace, command, refusal)
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
// decision above — ADR-038's own amendment paragraph) — moved to its own
// file under the R8 LOC cap; see `agent_call/fence.rs`. `fence_scraped_fields`
// is the one entry point `dispatch_direct` (below) and `reshape.rs`/`proof.rs`
// (their own `use super::*`) call.
mod fence;
use fence::fence_scraped_fields;

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
    let data = reshape_reply(command, data, page_args);
    // A3-r1-AC-7 — a direct read of the grace window's own read command is exactly the recovery
    // path a `ConfirmationRequired` refusal's hint sends a caller to; refreshing here closes the
    // double-drift gap a single confirmation_required-time snapshot can't (see `proof::
    // refresh_from_read`'s own doc). A no-op for every command but that one.
    proof::refresh_from_read(command, &data);
    Ok(data)
}

/// The whole decision [`dispatch_irreversible_confirmed`] makes, with the
/// `AppHandle` factored out — the impure wrapper below only resolves the
/// proof and hands the result here, so the ceremony's two refusal paths are
/// directly testable (the crate has no Tauri mock, so nothing taking a
/// concrete `&AppHandle` can be).
///
/// `run` is called at most ONCE and ONLY on an accepted `confirm` — never
/// before the comparison, which is the property the tests mutation-check: a
/// version that ran first and compared after would dispatch an
/// irreversible command on a wrong `confirm`. It returns whatever the
/// caller's own run step produces (the async wrapper returns the UNAWAITED
/// future, so this core stays sync and pure).
///
/// The comparison itself is [`proof::accepted`] (issue #1162), not a bare `==`: an exact match on
/// the FRESH `resolved` value is still the ordinary case, but for a [`proof::grace_window_key`]-
/// eligible `source` (today, ONLY `ai_spend_summary`-backed rows — security review round A3-r1,
/// AC-1/SEC-1 CRITICAL), `confirm` may also match a snapshot [`super::dispatch`]/[`dispatch_direct`]
/// recorded, provided that snapshot's grace window hasn't closed — see that fn's own doc for why a
/// spend-based proof can legitimately move between disclosure and confirm. Every other row's
/// `source` has no grace window at all: only the exact fresh value is ever accepted.
fn confirm_and_run<T>(
    source: ProofSource,
    resolved: Option<String>,
    confirm: &str,
    run: impl FnOnce() -> T,
) -> Result<T, Refusal> {
    let expected = resolved.ok_or(Refusal::ProofUnavailable)?;
    match proof::accepted(proof::grace_window_key(source), &expected, confirm) {
        Ok(()) => Ok(run()),
        Err(proof::SnapshotOutcome::Mismatch) => {
            Err(Refusal::ConfirmationMismatch { moved: false })
        }
        Err(proof::SnapshotOutcome::Expired) => Err(Refusal::ConfirmationMismatch { moved: true }),
    }
}

/// Dispatch an `Irreversible` row whose `confirm` is already known to be
/// present (the caller — [`dispatch`] — only reaches here via
/// [`Dispatch::Confirmed`], produced by [`gate`]): resolve the expected
/// value FRESH via [`proof::resolve`] and only then run the real command.
/// A thin wrapper over [`confirm_and_run`] — the only thing that needs the
/// `AppHandle` is the resolve and the dispatch themselves.
///
/// ponytail: the resolved proof (`source`) is derived from the TARGET record alone (e.g.
/// `applications_delete` → `application.title`) and never from `input`, so it proves "you read
/// this record", never "you chose this SCOPE" — `applications_delete`'s `keepDocuments: false`
/// cascade (deletes every generated resume/cover letter + the run trail) is authorised by the
/// same proof string as `keepDocuments: true` (A1-r1-SEC-2 MEDIUM, deferred: binding the proof to
/// a destructive-branch flag is a wire-format change to the confirm ceremony itself — `proof::hint`,
/// the MCP `CONFIRMATION_NOTE`, and ADR-038 §4 all assume one proof string per record, not per
/// (record, flag) pair). Ceiling: today's ceremony proves READ only. Upgrade path: extend
/// `ProofSource::Lookup` with an optional scope-binding suffix keyed off a caller-input field (e.g.
/// `<title>|keepDocuments=false` on the cascade branch), thread it through `proof::extract`/`hint`,
/// and record the new confirm shape in ADR-038 §4 — tracked on issue #1160 (deferral + upgrade
/// path recorded in a comment there, A1-r2-SEC-1 MEDIUM), not implemented here.
async fn dispatch_irreversible_confirmed(
    app: &AppHandle,
    command: &str,
    input: Value,
    source: ProofSource,
    confirm: &str,
) -> Result<Value, Refusal> {
    let resolved = proof::resolve(app, source, &input).await;
    confirm_and_run(source, resolved, confirm, || {
        dispatch_direct(app, command, input)
    })?
    .await
}

async fn dispatch(
    app: &AppHandle,
    namespace: &str,
    command: &str,
    input: Value,
    confirm: Option<&str>,
) -> Result<Value, Refusal> {
    let entry = find_policy(namespace, command)
        .ok_or_else(|| Refusal::UnknownCommand(namespace_suggestion(command)))?;
    match plan(entry, command, &input, confirm) {
        Ok(Dispatch::Direct) => dispatch_direct(app, command, input).await,
        Ok(Dispatch::Confirmed { source, confirm }) => {
            dispatch_irreversible_confirmed(app, command, input, source, confirm).await
        }
        // Issue #1162 — snapshot the CURRENT proof value right NOW, at the moment this
        // `confirmation_required` refusal is issued: this is the earliest point a grace window
        // can start, and doing it here (rather than lazily on the retry) means a caller that
        // reads the disclosed field and pastes it straight back is comparing against a value
        // this process itself resolved, never one it could have influenced. Best-effort — a
        // `None` (the target record doesn't exist yet, or its own read failed) changes nothing
        // about the refusal the caller already gets; it just means no snapshot lands. Only for a
        // [`proof::grace_window_key`]-eligible `source` (security review round A3-r1, AC-1/SEC-1
        // CRITICAL) — every per-target row never gets a snapshot at all, so it can never be
        // satisfied by anything but the fresh, just-resolved value.
        Err(Refusal::ConfirmationRequired(hint)) => {
            if let Effect::Irreversible(source) = entry.effect {
                if let Some(key) = proof::grace_window_key(source) {
                    if let Some(value) = proof::resolve(app, source, &input).await {
                        proof::remember(key, value);
                    }
                }
            }
            Err(Refusal::ConfirmationRequired(hint))
        }
        Err(other) => Err(other),
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
