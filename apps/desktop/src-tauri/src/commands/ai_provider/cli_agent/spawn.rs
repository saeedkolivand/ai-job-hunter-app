//! Building and launching the child process: the per-spawn workspace wiring,
//! feeding the prompt on stdin without deadlocking, and the argv-token guard
//! that defends the CVE-2024-24576 invariant for user-settings values.

use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use super::{prepare_workspace, CliAgentBackend, CliInvocation, PromptDelivery, Workspace};

/// Spawn the CLI. The returned [`Workspace`] (if the backend has config files)
/// must be held until the child exits: dropping it deletes the config the child
/// reads.
pub(super) fn spawn(
    binary: &str,
    inv: &CliInvocation,
    prompt: &str,
    backend: &dyn CliAgentBackend,
) -> std::io::Result<(tokio::process::Child, Option<Workspace>)> {
    let mut args = inv.args.clone();
    // Untrusted prompt text enters argv ONLY for `PromptDelivery::Arg`, which no
    // backend constructs — every agent uses `Stdin` (see `PromptDelivery` docs for
    // the CVE-2024-24576 rationale). So in practice `args` is fixed trusted flags,
    // and the prompt reaches the child via `child.stdin` in the callers below.
    if matches!(inv.prompt, PromptDelivery::Arg) {
        args.push(prompt.to_string());
    }

    let files = backend.workspace_files();
    let workspace = if files.is_empty() {
        None
    } else {
        Some(prepare_workspace(
            &crate::platform::config::data_dir(),
            backend.id().as_str(),
            &files,
        )?)
    };

    let mut cmd = super::cli_command(binary, &args);
    match &workspace {
        Some(ws) => {
            cmd.current_dir(ws.path());
            for (var, rel_path) in backend.workspace_env() {
                cmd.env(var, ws.path().join(rel_path));
            }
        }
        None => {
            cmd.current_dir(std::env::temp_dir());
        }
    }
    let child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    Ok((child, workspace))
}

/// Feed the prompt to the child's stdin on a **detached task** so the caller can
/// drain stdout concurrently. Awaiting the full write *before* reading stdout can
/// DEADLOCK when the prompt exceeds the OS pipe buffer (~64 KB) and the child
/// interleaves stdout while still reading stdin: both pipes fill and neither side
/// progresses — surfacing only as the 5-minute timeout. This is realistic for
/// cover-letter prompts that inline the full scraped JD + résumé + research brief.
///
/// The task drops `stdin` when done so the child sees EOF (unchanged from before);
/// a write error (e.g. a CLI that closed stdin early) is logged at `debug`, never
/// fatal — success is still decided by the stdout/exit path. Only
/// [`PromptDelivery::Stdin`] pipes the prompt; for the unused
/// [`PromptDelivery::Arg`] the prompt already rode argv, so stdin is just closed.
pub(super) fn write_prompt_stdin(
    delivery: PromptDelivery,
    stdin: Option<tokio::process::ChildStdin>,
    prompt: String,
    label: &str,
) -> tokio::task::JoinHandle<()> {
    let label = label.to_string();
    tokio::spawn(async move {
        // No piped stdin (already taken) → nothing to do.
        let Some(mut stdin) = stdin else { return };
        // `Arg` (unused) already carried the prompt on argv; drop `stdin` to close it
        // (EOF) without writing — matches the old `drop(child.stdin.take())`.
        if delivery != PromptDelivery::Stdin {
            return;
        }
        if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
            tracing::debug!("[cli_agent] {label}: stdin write ended early: {e}");
        }
        // `stdin` drops here → EOF, so the agent stops waiting for input.
    })
}

/// Defense-in-depth for the CVE-2024-24576 invariant (argv carries only fixed,
/// trusted flags). `model`/`effort` come from the user's OWN settings — never
/// scraped/untrusted content — but on Windows they still ride argv through the
/// `cmd.exe /C` wrapper, where Rust's batch-escaping does not engage. So before a
/// backend turns one into an arg, require it to be a plain identifier
/// (`[A-Za-z0-9._:-]+`, trimmed): anything with a shell metacharacter, whitespace,
/// or control char is dropped (the flag is simply omitted, so the CLI falls back to
/// its default) and a warning is logged. Every real model id / effort level passes
/// unchanged. Also reject values starting with `-` or `/` so a settings value can
/// never be read as a flag or a cmd switch.
pub(super) fn arg_token(value: &str) -> Option<&str> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    // Reject leading `-` (looks like a flag) or `/` (cmd switch on Windows)
    if v.starts_with('-') || v.starts_with('/') {
        tracing::warn!("[cli_agent] dropping CLI arg value with leading flag/switch: {v:?}");
        return None;
    }
    if v.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-' | b'/'))
    {
        Some(v)
    } else {
        tracing::warn!("[cli_agent] dropping non-identifier CLI arg value: {v:?}");
        None
    }
}
