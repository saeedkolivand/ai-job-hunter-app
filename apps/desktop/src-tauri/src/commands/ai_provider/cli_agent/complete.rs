//! The one-shot engine (pipeline `complete` + native structured output):
//! spawn, feed stdin, await the full output, then parse it.

use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliInvocation};

pub(super) async fn run_complete(
    app: &AppHandle,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    user: &str,
) -> AppResult<String> {
    // Plain `complete` carries no effort: the agent's default effort.
    run_complete_with_effort(app, backend, model, system, user, None).await
}

/// [`run_complete`] at an explicit effort — the one-shot half of
/// `AiProvider::complete_with_effort`. Each backend's own
/// `complete_invocation` decides whether it reads the value (only Codex and
/// Claude Code do; the rest ignore it).
pub(super) async fn run_complete_with_effort(
    app: &AppHandle,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    user: &str,
    effort: Option<&str>,
) -> AppResult<String> {
    let inv = backend.complete_invocation(model, system, effort);
    run_one_shot(app, backend, model, system, user, inv, |b, stdout| {
        b.parse_complete(stdout)
    })
    .await
}

/// One-shot STRUCTURED completion — the native half of
/// [`super::CliAgentClient::complete_structured`]: the backend's prebuilt
/// [`CliAgentBackend::native_json_schema_invocation`], parsed with
/// [`CliAgentBackend::parse_structured_complete`]. Returns plain text — the
/// caller pairs it with [`super::super::Usage::default`], because a CLI agent
/// reports no usage (same contract as `prompt_only`/`run_complete`).
pub(super) async fn run_structured_complete(
    app: &AppHandle,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    user: &str,
    inv: CliInvocation,
) -> AppResult<String> {
    run_one_shot(app, backend, model, system, user, inv, |b, stdout| {
        b.parse_structured_complete(stdout)
    })
    .await
}

/// The spawn → stdin → timeout → [`super::friendly_cli_error`] → parse flow both
/// one-shot paths share; they differ only in the invocation and in how stdout
/// is parsed.
async fn run_one_shot(
    app: &AppHandle,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    user: &str,
    inv: CliInvocation,
    parse: fn(&dyn CliAgentBackend, &str) -> AppResult<String>,
) -> AppResult<String> {
    let _ = app; // CLI agents resolve everything from the binary; no managed state needed.
    let binary = backend.binary();
    let label = backend.id().as_str();
    let prompt = super::effective_prompt(backend, system, user);
    let trace = super::RequestTrace::begin(backend.id(), model, "cli:complete", &binary, false);

    // `_workspace` (not `_`) keeps the per-spawn config dir alive until this
    // function returns, i.e. until the child has exited.
    let (mut child, _workspace) = match super::spawn(&binary, &inv, &prompt, backend) {
        Ok(spawned) => spawned,
        Err(e) => {
            trace.end(None, false);
            return Err(super::spawn_error(label, &binary, e));
        }
    };

    // Feed stdin on a detached task so `wait_with_output` (which drains stdout/stderr)
    // runs concurrently — awaiting the full write first can DEADLOCK on a prompt
    // larger than the OS pipe buffer (~64 KB); see `write_prompt_stdin`.
    let stdin_writer = super::write_prompt_stdin(inv.prompt, child.stdin.take(), prompt, label);

    let output = match tokio::time::timeout(super::TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            trace.end(None, false);
            return Err(AppError::Provider(format!("{label}: {e}")));
        }
        Err(_) => {
            trace.end(None, false);
            return Err(AppError::Network(format!(
                "{label} timed out after 5 minutes."
            )));
        }
    };
    // Reap the stdin writer — the child has exited, so it has completed.
    let _ = stdin_writer.await;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        trace.end(output.status.code().map(|c| c as u16), false);
        return Err(super::friendly_cli_error(
            label,
            output.status.code(),
            &stderr,
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = parse(backend, &stdout)?;
    trace.end(output.status.code().map(|c| c as u16), true);
    Ok(text)
}
