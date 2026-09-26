//! Every reason `agent.call` dispatch refuses — the `Refusal` vocabulary and its sentinel
//! strings. The `sentinel()`/`detail()` methods (the wire string and the human/agent-readable
//! prose per variant) are an inherent `impl` in `refusal/detail.rs`, split out under the same R8
//! LOC cap.

mod detail;

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
pub(in crate::extension_bridge) enum Refusal {
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
    /// App state this dispatch needed to read (today: [`stored_profile_value`]'s
    /// pre-write read) failed — a store I/O/parse error, never the target
    /// command's own dispatch. P-r2-AC-R5-F4: used to fold into
    /// [`Refusal::DispatchFailed`], whose doc guarantees a fixed,
    /// framework-only string, making that guarantee false.
    StateUnreadable(String),
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
    /// The extension caller's own consent gate (PR1, decision 2): an `agent.call` from the
    /// paired EXTENSION while Assisted autofill is off. Never applies to the CLI — it has no such
    /// gate — nor to [`Refusal::OriginRefused`]'s case (that caller failed origin resolution
    /// entirely); this is a THIRD caller class's own refusal, checked only once origin AND effect
    /// would otherwise allow the call through.
    ExtensionReadGate,
    /// The extension caller reached a row whose [`Effect`] is NOT `Read` (PR1, decision 1): the
    /// extension tier is Read-only by construction — checked PURELY off [`POLICY`], before any
    /// I/O — so a Reversible/Irreversible/NotExposed row refuses HERE, never entering the confirm
    /// ceremony (no `proof_field_for`, no hint about which `Resource` yields a proof). That
    /// ceremony is built for a CLI/LLM caller issuing two sequential calls itself (`agent_cli.rs`'s
    /// own doc); collapsing it into an extension-driven two-hop flow with no human click between
    /// would be the exact one-hop self-authenticating shape ADR-040 already flags as unsafe.
    EffectNotAllowedForExtension,
}

/// `pub(super)` — the MCP server's `call-*` tools refuse locally with this
/// SAME sentinel for a `<namespace>:<command>` their own bundled `POLICY`
/// copy doesn't know, rather than a second hand-typed literal (same
/// reasoning as [`ERR_CONFIRMATION_REQUIRED`]'s own doc).
pub(in crate::extension_bridge) const ERR_UNKNOWN_COMMAND: &str = "unknown_command";
/// [`Refusal::InvalidInput`]'s sentinel. `pub(super)` (A1-r1-SEC-1 HIGH) — the MCP server's
/// `local_call_refusal` now runs [`invalid_input_detail`] locally too, refusing with this SAME
/// sentinel rather than trusting the peer app's own dispatch-time check.
pub(in crate::extension_bridge) const ERR_INVALID_INPUT: &str = "invalid_input";
/// `pub(super)` — the MCP server refuses a `NotExposed` row LOCALLY with this
/// SAME sentinel (so the token-row fix does not depend on the peer app's build;
/// see `agent_cli::mcp::local_call_refusal`), never a second hand-typed copy.
pub(in crate::extension_bridge) const ERR_NOT_EXPOSED: &str = "not_exposed";
const ERR_CLI_ONLY: &str = "cli_only";
pub(in crate::extension_bridge) const ERR_RATE_LIMITED: &str = "rate_limited"; // reused by agent_read, issue #1155
const ERR_DISPATCH_FAILED: &str = "dispatch_failed";
/// Distinct from [`ERR_DISPATCH_FAILED`] — see [`Refusal::StateUnreadable`].
const ERR_STATE_UNREADABLE: &str = "state_unreadable";
const ERR_INVOKE_ERROR: &str = "invoke_error";
/// `pub(super)` — [`super::agent_cli::exit_code_for_reply`] matches on this
/// EXACT sentinel to special-case exit 4, never a second hand-typed copy of
/// the string.
pub(in crate::extension_bridge) const ERR_CONFIRMATION_REQUIRED: &str = "confirmation_required";
const ERR_CONFIRMATION_MISMATCH: &str = "confirmation_mismatch";
const ERR_PROOF_UNAVAILABLE: &str = "proof_unavailable";
/// `pub(super)` — the MCP server's own, much smaller result cap
/// (`agent_cli::mcp::oversized_result`) refuses with this SAME sentinel one
/// hop further out. One cause, one name, ONE definition of the string: two
/// hand-typed copies of a sentinel is the drift this module's own
/// [`ERR_CONFIRMATION_REQUIRED`] doc already argues against.
pub(in crate::extension_bridge) const ERR_RESULT_TOO_LARGE: &str = "result_too_large";
pub(super) const ERR_INVALID_CURSOR: &str = "invalid_cursor";
/// `pub(super)` — reused verbatim by `agent_read::extension_gate_reply` for the IDENTICAL gate on
/// its own tier (PR1), so both surfaces report the same sentinel for the same cause.
pub(in crate::extension_bridge) const ERR_EXTENSION_READ_GATE: &str = "extension_read_gate";
pub(super) const ERR_EFFECT_NOT_ALLOWED_FOR_EXTENSION: &str = "effect_not_allowed_for_extension";

/// Fixed sentinel — mirrors `agent_read::CLI_ONLY_MESSAGE` for the identical
/// gate, applied to the generic tier's own wire type.
const CLI_ONLY_MESSAGE: &str = "agent.call is only available to the ajh-tauri agent CLI";
