//! `Refusal`'s wire sentinel and human/agent-readable detail prose — split out of `refusal.rs`
//! under the same R8 LOC-cap reasoning as that file's own split from `agent_call.rs`.

use super::*;

impl Refusal {
    pub(in crate::extension_bridge::agent_call) fn sentinel(&self) -> &'static str {
        match self {
            Refusal::UnknownCommand(_) => ERR_UNKNOWN_COMMAND,
            Refusal::InvalidInput(_) => ERR_INVALID_INPUT,
            Refusal::NotExposed(_) => ERR_NOT_EXPOSED,
            Refusal::OriginRefused => ERR_CLI_ONLY,
            Refusal::RateLimited { .. } => ERR_RATE_LIMITED,
            Refusal::DispatchFailed(_) => ERR_DISPATCH_FAILED,
            Refusal::StateUnreadable(_) => ERR_STATE_UNREADABLE,
            Refusal::InvokeError(_) => ERR_INVOKE_ERROR,
            Refusal::ConfirmationRequired(_) => ERR_CONFIRMATION_REQUIRED,
            Refusal::ConfirmationMismatch { .. } => ERR_CONFIRMATION_MISMATCH,
            Refusal::ProofUnavailable => ERR_PROOF_UNAVAILABLE,
            Refusal::ResultTooLarge(_) => ERR_RESULT_TOO_LARGE,
            Refusal::InvalidCursor => ERR_INVALID_CURSOR,
            Refusal::ExtensionReadGate => ERR_EXTENSION_READ_GATE,
            Refusal::EffectNotAllowedForExtension => ERR_EFFECT_NOT_ALLOWED_FOR_EXTENSION,
        }
    }

    /// Human/agent-readable detail. [`Refusal::ConfirmationRequired`] and
    /// [`Refusal::ConfirmationMismatch`] never carry the proof VALUE — see
    /// each variant's own doc; this is the one place both are rendered, so
    /// it is also the one place that guarantee could be broken, hence the
    /// dedicated tests in `agent_call::tests`.
    ///
    /// [`Refusal::InvokeError`]'s `detail` carries the explanatory prose UNLABELLED but fences
    /// ONLY the underlying value under the distinct `<command_error>` tag (SEC-1 fix, issue
    /// #1157): most of the time the value is the app's OWN Tauri argument-validation sentence (a
    /// missing/mistyped arg, an ACL denial, an unregistered command) -- the single most actionable
    /// line an agent gets anywhere on this surface, and wrapping it as `<job_posting>` markup once
    /// made a first-party diagnostic read as though a job board had written it (round 4's mistake)
    /// -- but the SAME two causes are wire-indistinguishable (`agent_call.rs`'s own module doc),
    /// and the command-error cause CAN embed third-party/remote text (a scrape/HTTP/provider
    /// failure echoing part of a caller-chosen host's own response, e.g. `ai_pull_model`'s Ollama
    /// body or a provider's raw error message). An earlier revision left the value entirely
    /// unfenced to avoid the `job_posting` mislabel, which also dropped the "treat as data" label
    /// from a field that can carry attacker-influenced prose on a surface whose caller holds
    /// destructive tools (round A3-r1 SEC-1 HIGH). `<command_error>` -- registered in
    /// `crate::prompt_fence`'s fence-tag registry and capped at [`crate::prompt_fence::JOB_CAP`]
    /// chars -- is the same mixed-provenance remedy `agent_call::fence`'s `app_notification` tag
    /// already gives notification copy: framed as DATA, never asserted third-party, and still
    /// covered by [`crate::prompt_fence::neutralize_transcript_boundaries`]'s forged-boundary
    /// defence (round A3-r1 AC-2/SEC-3 HIGH) either way.
    pub(in crate::extension_bridge::agent_call) fn detail(&self) -> String {
        match self {
            Refusal::UnknownCommand(suggestion) => {
                super::super::policy_lookup::unknown_command_detail(*suggestion)
            }
            Refusal::InvalidInput(detail) => detail.clone(),
            Refusal::NotExposed(reason) => format!("not exposed to any CLI tier: {reason}"),
            Refusal::OriginRefused => CLI_ONLY_MESSAGE.to_string(),
            Refusal::RateLimited { .. } => {
                super::super::super::agent_read::THROTTLED_MESSAGE.to_string()
            }
            Refusal::DispatchFailed(detail) => detail.clone(),
            Refusal::StateUnreadable(detail) => {
                format!("could not read app state this dispatch needed: {detail}")
            }
            Refusal::InvokeError(detail) => {
                // The explanatory prose stays unlabelled; only the underlying value is fenced,
                // under the distinct `command_error` tag -- see this variant's own `detail()` doc
                // above (SEC-1 fix, issue #1157).
                let fenced = crate::prompt_fence::fenced(
                    "command_error",
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
            Refusal::InvalidCursor => {
                super::super::super::paging::INVALID_CURSOR_MESSAGE.to_string()
            }
            Refusal::ExtensionReadGate => EXTENSION_READ_GATE_MESSAGE.to_string(),
            Refusal::EffectNotAllowedForExtension => {
                EFFECT_NOT_ALLOWED_FOR_EXTENSION_MESSAGE.to_string()
            }
        }
    }
}

/// [`Refusal::ExtensionReadGate`]'s detail — mirrors `agent_read::EXTENSION_READ_GATE_DETAIL`
/// (same gate, same wording, one wire type over); kept as a separate literal (not threaded across
/// the module boundary) since the two files already keep their own `detail` prose independent
/// (compare `CLI_ONLY_MESSAGE` in each), while the SENTINEL is what both actually reuse
/// ([`ERR_EXTENSION_READ_GATE`]).
const EXTENSION_READ_GATE_MESSAGE: &str =
    "Turn on Assisted autofill in AI Job Hunter → Settings → Browser extension to let \
     the paired browser extension read your data.";
/// [`Refusal::EffectNotAllowedForExtension`]'s detail.
const EFFECT_NOT_ALLOWED_FOR_EXTENSION_MESSAGE: &str =
    "the browser extension may only reach Read-effect commands through agent.call — this command \
     is not exposed to it";
