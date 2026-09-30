//! Translating a failed spawn or a non-zero CLI exit into a friendly
//! [`AppError`], including the specific auth / stale-CLI shapes worth a
//! better message than the raw stderr dump.

use crate::error::AppError;

pub(super) fn spawn_error(agent: &str, binary: &str, e: std::io::Error) -> AppError {
    if e.kind() == std::io::ErrorKind::NotFound {
        AppError::Config(format!(
            "{agent} CLI not found (looked for '{binary}'). Install it or set its binary path."
        ))
    } else {
        AppError::Provider(format!("Failed to start {agent}: {e}"))
    }
}

/// Which error a finished CLI-agent stream reports when `emit_done` rejected an
/// empty answer.
///
/// An agent that emitted a `Done` sentinel (or a whitespace-only delta) and THEN
/// exited non-zero skips `run_stream`'s earlier `!emitted_done && !success` guard,
/// so without this it reported the generic "produced no answer content" and threw
/// away the stderr that says why — the not-logged-in / quota / bad-flag cases
/// [`friendly_cli_error`] already recognises. The captured stderr is strictly more
/// informative than the empty-answer message whenever the process itself failed;
/// on a CLEAN exit there is no stderr diagnosis to prefer, so the empty-answer
/// message stands.
///
/// Split out from `run_stream` purely so it is testable: `run_stream` needs an
/// `AppHandle` and a real child process, this decision needs neither.
pub(super) fn terminal_error(
    empty_answer: AppError,
    success: bool,
    agent: &str,
    code: Option<i32>,
    stderr: &str,
) -> AppError {
    if success {
        empty_answer
    } else {
        friendly_cli_error(agent, code, stderr)
    }
}

pub(super) fn friendly_cli_error(agent: &str, code: Option<i32>, stderr: &str) -> AppError {
    let s = stderr.to_ascii_lowercase();
    if s.contains("not logged in")
        || s.contains("unauthorized")
        || s.contains("authentication")
        || s.contains("not authenticated")
        || s.contains("please log in")
        || s.contains("/login")
    {
        return AppError::Config(format!(
            "{agent} is installed but not signed in. Run it once in a terminal to log in."
        ));
    }
    // An old Codex build that predates the isolation flags (Part 1d) rejects
    // them via clap: "error: unexpected argument '--ignore-user-config' found".
    // That is out-of-band CLI drift, not a config problem — say so instead of
    // dumping the raw clap text. Scoped to codex (`agent` is
    // `ProviderId::as_str`) so another agent's genuine "unexpected argument"
    // can't be mislabeled as an update prompt.
    if agent == "codex" && s.contains("unexpected argument") {
        return AppError::Provider(
            "codex CLI is too old for this app's isolation flags (it rejected one as \
             an \"unexpected argument\"). Update the Codex CLI (npm install -g \
             @openai/codex) and try again."
                .to_string(),
        );
    }
    let detail: String = stderr.trim().chars().take(300).collect();
    let code_str = code.map(|c| format!(" (exit {c})")).unwrap_or_default();
    if detail.is_empty() {
        AppError::Provider(format!("{agent} failed{code_str}."))
    } else {
        AppError::Provider(format!("{agent}{code_str}: {detail}"))
    }
}
