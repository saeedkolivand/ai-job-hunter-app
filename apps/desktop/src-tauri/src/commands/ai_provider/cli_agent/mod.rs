//! CLI-agent provider family.
//!
//! A *CLI agent* is a locally-installed coding agent (Claude Code, OpenAI Codex,
//! Gemini CLI…) run **headless** as a subprocess. It authenticates with its **own**
//! login (Claude Pro/Max, ChatGPT, Google) — there is no API key in this app, and
//! no outbound HTTP from us; we spawn the binary, feed it a prompt, and stream its
//! stdout.
//!
//! Everything tool-specific lives behind the [`CliAgentBackend`] trait (binary,
//! flags, output parsing, model aliases). The spawning / streaming / cancellation /
//! timeout / detection engine here is shared, and [`CliAgentClient`] adapts any
//! backend to the centralized [`AiProvider`] trait. Adding an agent = one
//! `CliAgentBackend` impl + one entry in [`all`] — routing
//! ([`super::resolve`]) and detection (`system_health`) read that list, so they
//! never change.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM};
use crate::jobs::JobTracker;
use crate::platform::NoWindow;

use super::research;
use super::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace, TokenParam,
};

mod antigravity;
mod claude_code;
mod codex;
mod gemini_cli;

use antigravity::AntigravityAgent;
use claude_code::ClaudeCodeAgent;
use codex::CodexAgent;
use gemini_cli::GeminiCliAgent;

/// Max wall-clock time for a single CLI generation before we kill the child.
const TIMEOUT: Duration = Duration::from_secs(300);

/// How often the stream loop re-polls the JobTracker for cancellation while
/// waiting on the next output line, so a cancel mid-line (or on a stalled stream)
/// is observed promptly instead of blocking until the next newline arrives.
const CANCEL_POLL: Duration = Duration::from_millis(200);

// ── Agent contract ──────────────────────────────────────────────────────────────

/// A neutral, parsed event from a CLI agent's output stream. Each backend's
/// [`CliAgentBackend::parse_stream_line`] maps one raw output line to this, keeping
/// all per-tool JSON knowledge in one pure, unit-testable function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliEvent {
    /// Assistant text to append to the response.
    Delta(String),
    /// Extended-thinking text (surfaced separately, like the cloud providers).
    Thinking(String),
    /// The agent finished successfully.
    Done,
    /// The agent reported a fatal error.
    Error(String),
}

/// How the prompt reaches the child process.
///
/// **Security — CVE-2024-24576 (Windows `.cmd`/"BatBadBut"):** the prompt inlines
/// untrusted, scraped job-description text. On Windows an npm-global CLI
/// (`codex`/`gemini`/`agy`) installs as a `.cmd` shim that must be launched through
/// `cmd.exe /C` ([`cli_command`]). Rust's batch-argument escaping (the CVE fix) only
/// engages when the spawned program is the `.cmd` itself — here the program is
/// `cmd.exe`, not the `.cmd`, so it does NOT engage. A prompt containing `" & <cmd>`
/// would then break out of `cmd.exe`'s parser and execute. The structural fix:
/// **untrusted text must never transit argv.** Every backend uses
/// [`Stdin`](Self::Stdin), so the command line carries only fixed, trusted flags and
/// the harness pipes the prompt to `child.stdin`. Fail-closed — a CLI that ignores
/// stdin yields empty output, never RCE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptDelivery {
    /// Pipe the prompt to stdin (avoids arg-length / escaping limits, and keeps
    /// untrusted prompt bytes off the command line — see the type-level security
    /// note). The only variant any backend uses.
    Stdin,
    /// Append the prompt as the **final** argv element. **Unused** — retained only
    /// as harness plumbing. Do NOT adopt it for any backend whose binary can be a
    /// Windows `.cmd` shim: it would route untrusted prompt text through `cmd.exe`
    /// (CVE-2024-24576, see above). Prefer [`Stdin`](Self::Stdin).
    #[allow(dead_code)]
    Arg,
}

/// A fully-built subprocess invocation (everything except the prompt, which the
/// harness delivers per [`PromptDelivery`]).
pub struct CliInvocation {
    pub args: Vec<String>,
    pub prompt: PromptDelivery,
}

/// One CLI coding agent. Implementors are tiny: identity, how to invoke the binary,
/// and how to parse its output.
#[async_trait]
pub trait CliAgentBackend: Send + Sync {
    fn id(&self) -> ProviderId;
    /// Default binary name, looked up on `PATH` (e.g. `"claude"`).
    fn default_binary(&self) -> &'static str;
    /// Env var that overrides the binary path (e.g. `"CLAUDE_CODE_BIN"`).
    fn env_override(&self) -> &'static str;
    /// Model aliases offered in the UI (e.g. `["sonnet", "opus", "haiku"]`).
    fn models(&self) -> &'static [&'static str];

    /// The npm package that provides this agent's binary, for the in-app install
    /// (#22). The one-click install runs `npm install -g <this>` — and that exact
    /// command MUST also be present in the shell capability allowlist
    /// (`capabilities/default.json`); a test asserts the two agree.
    fn install_package(&self) -> &'static str;

    /// Official install / setup docs, opened by the "guide" path.
    fn docs_url(&self) -> &'static str;

    /// Args for a streaming generation (model/system already resolved). `effort` is
    /// the optional reasoning effort for agents that support it (Codex); others ignore it.
    fn stream_invocation(&self, model: &str, system: &str, effort: Option<&str>) -> CliInvocation;
    /// Args for a one-shot, non-streaming generation.
    fn complete_invocation(&self, model: &str, system: &str, effort: Option<&str>)
        -> CliInvocation;

    /// Map one raw stdout line to a [`CliEvent`], or `None` to ignore it.
    fn parse_stream_line(&self, line: &str) -> Option<CliEvent>;
    /// Extract the final assistant text from a one-shot invocation's full stdout.
    fn parse_complete(&self, stdout: &str) -> AppResult<String>;

    /// Resolved binary path: env override, else [`default_binary`](Self::default_binary).
    fn binary(&self) -> String {
        crate::platform::config::env_override(self.env_override())
            .unwrap_or_else(|| self.default_binary().to_string())
    }

    /// Whether the harness should prepend the system prompt onto the user prompt.
    /// `false` (default) means the agent takes the system prompt via a flag in its
    /// invocation (e.g. Claude Code's `--append-system-prompt`); `true` is for
    /// agents with no system-prompt flag (Codex, Gemini CLI).
    fn inline_system(&self) -> bool {
        false
    }
}

/// Combine system + user per the backend's [`inline_system`](CliAgentBackend::inline_system).
fn effective_prompt(backend: &dyn CliAgentBackend, system: &str, prompt: &str) -> String {
    if backend.inline_system() && !system.trim().is_empty() {
        format!("{system}\n\n{prompt}")
    } else {
        prompt.to_string()
    }
}

// ── Registry (single source of truth) ───────────────────────────────────────────

/// Every registered CLI agent. Routing and detection both read this — adding an
/// agent here is all that's needed to surface it.
pub fn all() -> Vec<Box<dyn CliAgentBackend>> {
    vec![
        Box::new(ClaudeCodeAgent),
        Box::new(CodexAgent),
        Box::new(GeminiCliAgent),
        Box::new(AntigravityAgent),
    ]
}

/// The backend for a provider id, if it is a CLI agent.
pub fn backend_for(id: ProviderId) -> Option<Box<dyn CliAgentBackend>> {
    all().into_iter().find(|b| b.id() == id)
}

// ── Provider adapter ─────────────────────────────────────────────────────────────

/// Adapts any [`CliAgentBackend`] to the centralized [`AiProvider`] trait.
pub struct CliAgentClient {
    backend: Box<dyn CliAgentBackend>,
}

impl CliAgentClient {
    pub fn new(backend: Box<dyn CliAgentBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl AiProvider for CliAgentClient {
    fn id(&self) -> ProviderId {
        self.backend.id()
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            // The agent owns sampling/system handling; we pass system via a flag.
            supports_temperature: false,
            supports_system_role: false,
            supports_streaming: true,
            supports_reasoning: true,
            supports_tools: false,
            supports_json_mode: false,
            supports_embeddings: false,
            // The CLI agent carries its own web tools in headless mode.
            supports_web_search: true,
            token_param: TokenParam::MaxTokens,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let system = system_text(req);
        let prompt = user_prompt(req);
        run_stream(
            app,
            job_id,
            self.backend.as_ref(),
            &req.model,
            &system,
            &prompt,
            req.effort.as_deref(),
        )
        .await
    }

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        _temperature: Option<f64>,
    ) -> AppResult<String> {
        run_complete(app, self.backend.as_ref(), model, system, user).await
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        // CLI agents carry their own web tools — prompt them to search and write
        // the brief. Best-effort: any failure (or an agent without web access in
        // headless mode) degrades to "" so generation still proceeds.
        let user = research::native_user(company, role);
        Ok(run_complete(
            app,
            self.backend.as_ref(),
            model,
            research::NATIVE_SYSTEM,
            &user,
        )
        .await
        .unwrap_or_default())
    }

    #[allow(clippy::too_many_arguments)]
    async fn research_salary(
        &self,
        app: &AppHandle,
        model: &str,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        // Same best-effort contract as `research`: the agent's own web tools
        // search, `run_complete` degrades any failure to "" so generation
        // always proceeds.
        let user = research::salary_user(role, company, location, country, currency);
        Ok(run_complete(
            app,
            self.backend.as_ref(),
            model,
            &research::salary_system(currency),
            &user,
        )
        .await
        .unwrap_or_default())
    }

    async fn research_answer(
        &self,
        app: &AppHandle,
        model: &str,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        // Same best-effort contract as `research`/`research_salary`: the
        // agent's own web tools search, `run_complete` degrades any failure to
        // "" so generation always proceeds.
        let user = research::answer_user(question, role, company);
        Ok(run_complete(
            app,
            self.backend.as_ref(),
            model,
            research::ANSWER_SYSTEM,
            &user,
        )
        .await
        .unwrap_or_default())
    }

    async fn embed(&self, _app: &AppHandle, _model: &str, _text: &str) -> AppResult<Vec<f64>> {
        Err(AppError::Provider(format!(
            "{} has no embeddings API. Use OpenAI, Gemini, or Ollama for embeddings.",
            self.backend.id().as_str()
        )))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        None
    }

    async fn list_models(&self, _app: &AppHandle) -> Vec<Value> {
        self.backend
            .models()
            .iter()
            .map(|m| json!({ "name": m }))
            .collect()
    }

    async fn test_key(&self, _app: &AppHandle) -> AppResult<()> {
        let (ok, _version) = detect(&self.backend.binary()).await;
        if ok {
            Ok(())
        } else {
            Err(AppError::Config(format!(
                "{} CLI not found. Install it or set {}.",
                self.backend.id().as_str(),
                self.backend.env_override()
            )))
        }
    }
}

// ── Detection ────────────────────────────────────────────────────────────────────

/// How long a `<binary> --version` result is trusted before re-probing. Install
/// status changes rarely, so [`detect_cached`] collapses the 5 s health poll from
/// a subprocess spawn per tick to at most one per binary per TTL.
const DETECT_TTL: Duration = Duration::from_secs(300);

struct Detected {
    ok: bool,
    version: Option<String>,
    at: Instant,
}

fn detect_cache() -> &'static Mutex<HashMap<String, Detected>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Detected>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Base command for a CLI agent, with `args` applied: console window hidden on
/// Windows, and an augmented `PATH` so a GUI-launched macOS/Linux app can find the
/// binary (Finder/Dock apps inherit only a minimal `PATH`, missing
/// npm/nvm/Homebrew/native installs).
///
/// On **Windows** it first resolves the binary on `PATH` × `PATHEXT`
/// ([`crate::platform::resolve_cli_binary`]): a `.cmd`/`.bat` shim (how npm-global
/// CLIs like `gemini`/`codex`/`agy` install — there is no `.exe`) is launched
/// through `cmd.exe /C`, since `CreateProcess` cannot execute a batch file
/// directly. Each element of `args` is passed as a **separate argv entry** — never
/// concatenated into a shell string — so `cmd.exe` performs no word-splitting. If
/// resolution fails we fall through to a bare spawn so the OS still surfaces
/// `NotFound`.
///
/// **CVE-2024-24576 invariant:** because the program spawned is `cmd.exe` (not the
/// `.cmd`), Rust's batch-argument escaping does NOT engage — so `args` here MUST
/// carry only fixed, trusted flags (model/sandbox flags, `--version`), never
/// untrusted prompt/JD text. That text goes on stdin via [`PromptDelivery::Stdin`];
/// [`spawn`] only appends to argv for the unused [`PromptDelivery::Arg`], which no
/// backend constructs. See the [`PromptDelivery`] type docs.
fn cli_command(binary: &str, args: &[String]) -> Command {
    #[cfg(windows)]
    if let Some(resolved) = crate::platform::resolve_cli_binary(binary) {
        let mut cmd = if resolved.needs_cmd_wrapper {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&resolved.path).args(args);
            c
        } else {
            let mut c = Command::new(&resolved.path);
            c.args(args);
            c
        };
        cmd.no_window();
        return cmd;
    }

    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.no_window();
    if let Some(path) = crate::platform::cli_path() {
        cmd.env("PATH", path);
    }
    cmd
}

/// Whether the binary is installed (`<binary> --version` succeeds), plus its
/// reported version. Mirrors `ollama::reachable_model()` as the health signal.
pub async fn detect(binary: &str) -> (bool, Option<String>) {
    let fut = cli_command(binary, &["--version".to_string()]).output();
    match tokio::time::timeout(Duration::from_secs(5), fut).await {
        Ok(Ok(out)) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (true, (!v.is_empty()).then_some(v))
        }
        _ => (false, None),
    }
}

/// Cached [`detect`]: re-probes a binary at most once per [`DETECT_TTL`], so the
/// recurring `system_health` poll stops spawning a subprocess every few seconds.
/// The lock is released before the `.await`, never held across it.
pub async fn detect_cached(binary: &str) -> (bool, Option<String>) {
    {
        let cache = detect_cache().lock();
        if let Some(d) = cache.get(binary) {
            if d.at.elapsed() < DETECT_TTL {
                return (d.ok, d.version.clone());
            }
        }
    }
    let (ok, version) = detect(binary).await;
    detect_cache().lock().insert(
        binary.to_string(),
        Detected {
            ok,
            version: version.clone(),
            at: Instant::now(),
        },
    );
    (ok, version)
}

/// Drop all cached detection results so the next [`detect_cached`] re-probes.
/// Called right after an in-app install (#22) so freshly-installed agents show as
/// available immediately instead of after the [`DETECT_TTL`].
pub fn clear_detect_cache() {
    detect_cache().lock().clear();
}

// ── Streaming engine ─────────────────────────────────────────────────────────────

/// Outcome of one race between the next-line read and the cancel poll. A distinct
/// `Cancelled` variant keeps a natural EOF (`Eof`) from being misreported as a
/// cancel when cancellation and stream-end coincide in the same poll window —
/// overloading `Ok(None)` for both would surface a false "Job cancelled" error.
enum ReadOutcome {
    /// A complete line of agent output.
    Line(String),
    /// The stream ended cleanly (normal completion).
    Eof,
    /// Cancellation observed before the next line arrived.
    Cancelled,
    /// The underlying read failed.
    Err(std::io::Error),
}

async fn run_stream(
    app: &AppHandle,
    job_id: &str,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    prompt: &str,
    effort: Option<&str>,
) -> AppResult<()> {
    let binary = backend.binary();
    let label = backend.id().as_str();
    let inv = backend.stream_invocation(model, system, effort);
    let prompt = effective_prompt(backend, system, prompt);
    let trace = RequestTrace::begin(backend.id(), model, "cli:stream", &binary, true);

    let mut child = match spawn(&binary, &inv, &prompt) {
        Ok(c) => c,
        Err(e) => {
            trace.end(None, false);
            return Err(spawn_error(label, &binary, e));
        }
    };

    // Feed stdin on a detached task so the stdout loop below drains concurrently.
    // Awaiting the whole write first can DEADLOCK on a prompt larger than the OS
    // pipe buffer (~64 KB) if the child interleaves stdout while still reading stdin
    // (see `write_prompt_stdin`) — realistic for cover-letter prompts that inline
    // the full JD + résumé + research brief.
    let stdin_writer = write_prompt_stdin(inv.prompt, child.stdin.take(), prompt, label);

    // Drain stderr concurrently so a chatty agent can't deadlock on a full pipe.
    let stderr = child.stderr.take();
    let stderr_handle = tokio::spawn(async move {
        let mut buf = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut buf).await;
        }
        buf
    });

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Provider("Failed to capture CLI stdout".to_string()))?;
    let mut lines = BufReader::new(stdout).lines();

    let deadline = tokio::time::Instant::now() + TIMEOUT;
    let mut emitted_done = false;
    let mut any_delta = false;
    // Whether a non-blank content delta has streamed yet — gates leading-blank
    // suppression so plain-text agents don't emit a stray newline before the answer.
    let mut seen_content = false;

    loop {
        // Race the next line read against a cancel poll so a cancellation mid-line
        // (or on a stalled stream that never emits another newline) is observed
        // within `CANCEL_POLL` instead of hanging until the next line — or forever.
        // The poll branch loops back to re-check `is_cancelled`; the deadline still
        // bounds the whole read via `timeout_at`. A distinct `ReadOutcome` keeps a
        // real EOF from being misreported as a cancel when both happen to be true in
        // the same poll window (overloading `Ok(None)` for both would).
        let line = match tokio::time::timeout_at(deadline, async {
            loop {
                tokio::select! {
                    biased;
                    next = lines.next_line() => {
                        // Biased: a ready line always wins over the cancel poll.
                        break match next {
                            Ok(Some(l)) => ReadOutcome::Line(l),
                            Ok(None) => ReadOutcome::Eof,
                            Err(e) => ReadOutcome::Err(e),
                        };
                    }
                    _ = tokio::time::sleep(CANCEL_POLL) => {
                        if is_cancelled(app, job_id) {
                            break ReadOutcome::Cancelled;
                        }
                    }
                }
            }
        })
        .await
        {
            Err(_) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{label} timed out after 5 minutes."
                )));
            }
            Ok(ReadOutcome::Cancelled) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Message("Job cancelled".to_string()));
            }
            Ok(ReadOutcome::Line(line)) => line,
            Ok(ReadOutcome::Eof) => break, // clean EOF (never a cancel)
            Ok(ReadOutcome::Err(e)) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Provider(format!("{label}: read error: {e}")));
            }
        };

        // Don't pre-skip blank lines: JSON parsers return None for them anyway,
        // while plain-text agents (Gemini) rely on them for paragraph breaks.
        match backend.parse_stream_line(&line) {
            Some(CliEvent::Delta(text)) if !text.is_empty() => {
                any_delta = true;
                // Suppress leading blank/whitespace-only lines so streamed output
                // starts at the first real content line — matching the trimmed
                // one-shot (`parse_complete`) output. A blank line left after a
                // stripped credential notice (Gemini/Antigravity) would otherwise
                // stream a stray leading newline. `any_delta` is set above regardless,
                // so the success heuristic is unchanged.
                if !seen_content {
                    if text.trim().is_empty() {
                        continue;
                    }
                    seen_content = true;
                }
                emit_event(
                    app,
                    AI_STREAM,
                    AiStreamChunk {
                        job_id: job_id.to_string(),
                        delta: text,
                        done: false,
                        error: None,
                        thinking: None,
                    },
                );
            }
            Some(CliEvent::Thinking(text)) if !text.is_empty() => {
                emit_event(
                    app,
                    AI_STREAM,
                    AiStreamChunk {
                        job_id: job_id.to_string(),
                        delta: text,
                        done: false,
                        error: None,
                        thinking: Some(true),
                    },
                );
            }
            Some(CliEvent::Done) => {
                emitted_done = true;
                break;
            }
            Some(CliEvent::Error(msg)) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Provider(msg));
            }
            _ => {}
        }
    }

    let status = child.wait().await.ok();
    let success = status.map(|s| s.success()).unwrap_or(false);
    let stderr_text = stderr_handle.await.unwrap_or_default();
    // Reap the stdin writer — the child has exited, so it has completed; any write
    // error was already logged inside the task and is never fatal.
    let _ = stdin_writer.await;

    // No explicit terminal event (e.g. plain-text agents): a clean exit or any
    // streamed text means success; otherwise surface the failure.
    if !emitted_done && !success && !any_delta {
        trace.end(status.and_then(|s| s.code()).map(|c| c as u16), false);
        return Err(friendly_cli_error(
            label,
            status.and_then(|s| s.code()),
            &stderr_text,
        ));
    }

    // CLI agents run headless via their own tool's login (no API response to
    // read a `usage` field from) and stream over plain stdout lines, so they
    // never pass through the shared `commands::ai_provider::stream` loop that
    // records spend for the cloud adapters. Record zero tokens/cost here
    // (honest — never fabricate an estimate) so the AI-spend summary still
    // reflects that a call happened, at $0 real cost. The non-streaming
    // `complete`/`agent_run` path needs no equivalent call: it goes through
    // `AiProvider::complete_with_usage`'s DEFAULT impl, which already reports
    // zero usage for any provider (like this one) that doesn't override it.
    crate::spend::record_usage(app, backend.id().as_str(), model, 0, 0);
    emit_done(app, job_id);
    trace.end(status.and_then(|s| s.code()).map(|c| c as u16), true);
    Ok(())
}

// ── One-shot engine (pipeline `complete`) ────────────────────────────────────────

async fn run_complete(
    app: &AppHandle,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    user: &str,
) -> AppResult<String> {
    let _ = app; // CLI agents resolve everything from the binary; no managed state needed.
    let binary = backend.binary();
    let label = backend.id().as_str();
    // The non-streaming path runs at the agent's default effort (the request
    // carries no effort for `complete`).
    let inv = backend.complete_invocation(model, system, None);
    let prompt = effective_prompt(backend, system, user);
    let trace = RequestTrace::begin(backend.id(), model, "cli:complete", &binary, false);

    let mut child = match spawn(&binary, &inv, &prompt) {
        Ok(c) => c,
        Err(e) => {
            trace.end(None, false);
            return Err(spawn_error(label, &binary, e));
        }
    };

    // Feed stdin on a detached task so `wait_with_output` (which drains stdout/stderr)
    // runs concurrently — awaiting the full write first can DEADLOCK on a prompt
    // larger than the OS pipe buffer (~64 KB); see `write_prompt_stdin`.
    let stdin_writer = write_prompt_stdin(inv.prompt, child.stdin.take(), prompt, label);

    let output = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
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
        return Err(friendly_cli_error(label, output.status.code(), &stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = backend.parse_complete(&stdout)?;
    trace.end(output.status.code().map(|c| c as u16), true);
    Ok(text)
}

// ── Helpers ──────────────────────────────────────────────────────────────────────

fn spawn(
    binary: &str,
    inv: &CliInvocation,
    prompt: &str,
) -> std::io::Result<tokio::process::Child> {
    let mut args = inv.args.clone();
    // Untrusted prompt text enters argv ONLY for `PromptDelivery::Arg`, which no
    // backend constructs — every agent uses `Stdin` (see `PromptDelivery` docs for
    // the CVE-2024-24576 rationale). So in practice `args` is fixed trusted flags,
    // and the prompt reaches the child via `child.stdin` in the callers below.
    if matches!(inv.prompt, PromptDelivery::Arg) {
        args.push(prompt.to_string());
    }
    cli_command(binary, &args)
        // Neutral cwd: we only want text generation, never side effects in the
        // user's project (backends also disable tools where the CLI supports it).
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
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
fn write_prompt_stdin(
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
/// unchanged.
fn arg_token(value: &str) -> Option<&str> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        Some(v)
    } else {
        tracing::warn!("[cli_agent] dropping non-identifier CLI arg value: {v:?}");
        None
    }
}

fn spawn_error(agent: &str, binary: &str, e: std::io::Error) -> AppError {
    if e.kind() == std::io::ErrorKind::NotFound {
        AppError::Config(format!(
            "{agent} CLI not found (looked for '{binary}'). Install it or set its binary path."
        ))
    } else {
        AppError::Provider(format!("Failed to start {agent}: {e}"))
    }
}

fn friendly_cli_error(agent: &str, code: Option<i32>, stderr: &str) -> AppError {
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
    let detail: String = stderr.trim().chars().take(300).collect();
    let code_str = code.map(|c| format!(" (exit {c})")).unwrap_or_default();
    if detail.is_empty() {
        AppError::Provider(format!("{agent} failed{code_str}."))
    } else {
        AppError::Provider(format!("{agent}{code_str}: {detail}"))
    }
}

fn is_cancelled(app: &AppHandle, job_id: &str) -> bool {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .get(job_id)
        .map(|j| j.status == crate::jobs::JobStatus::Cancelled)
        .unwrap_or(false)
}

fn emit_done(app: &AppHandle, job_id: &str) {
    emit_event(
        app,
        AI_STREAM,
        AiStreamChunk {
            job_id: job_id.to_string(),
            delta: String::new(),
            done: true,
            error: None,
            thinking: None,
        },
    );
    crate::commands::jobs::job_complete(app, job_id, json!({ "done": true }));
}

/// All `system` message content, joined — passed to the agent as its system prompt.
fn system_text(req: &AiGenerateRequest) -> String {
    req.messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The non-system conversation as a single prompt. A lone user turn is passed
/// verbatim; multi-turn conversations are labelled so the agent keeps the thread.
fn user_prompt(req: &AiGenerateRequest) -> String {
    let turns: Vec<&_> = req.messages.iter().filter(|m| m.role != "system").collect();
    if turns.len() == 1 {
        return turns[0].content.clone();
    }
    turns
        .iter()
        .map(|m| {
            let label = if m.role == "assistant" {
                "Assistant"
            } else {
                "User"
            };
            format!("{label}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Plain-text CLI agents (Gemini CLI, Antigravity) interleave the model's answer
/// on stdout with a few operational lines — cached-credential notices, telemetry
/// banners, dotenv/deprecation logs. Left in, they get folded into the generated
/// letter. This recognizes those specific, exact-ish markers so a backend can drop
/// them. Deliberately conservative — it matches only known operational lines (full
/// exact matches or unmistakable log prefixes), never anything that could be real
/// answer text, and treats blank lines as paragraph breaks (kept).
fn is_cli_stdout_noise(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false; // paragraph break — never noise
    }
    // Full-line operational notices these CLIs print before/around the answer.
    const EXACT: &[&str] = &["Loaded cached credentials.", "Data collection is disabled."];
    if EXACT.contains(&t) {
        return true;
    }
    // Unmistakable log/tooling prefixes (dotenv injector banner, Node warnings that
    // some builds route to stdout). Prefix-anchored so ordinary prose can't match.
    t.starts_with("[dotenv@") || t.starts_with("DeprecationWarning:") || t.starts_with("(node:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_includes_all_cli_agents() {
        for id in [
            ProviderId::ClaudeCode,
            ProviderId::Codex,
            ProviderId::GeminiCli,
            ProviderId::Antigravity,
        ] {
            assert!(
                backend_for(id).is_some(),
                "{} should be registered",
                id.as_str()
            );
        }
        assert!(all().iter().all(|b| b.id().is_cli_agent()));
    }

    #[test]
    fn non_cli_provider_has_no_backend() {
        assert!(backend_for(ProviderId::Anthropic).is_none());
    }

    #[test]
    fn stdout_noise_filter_drops_only_operational_lines() {
        // Known operational noise is dropped…
        assert!(is_cli_stdout_noise("Loaded cached credentials."));
        assert!(is_cli_stdout_noise("  Data collection is disabled.  "));
        assert!(is_cli_stdout_noise(
            "[dotenv@17.0.0] injecting env (2) from .env"
        ));
        assert!(is_cli_stdout_noise("(node:12345) Warning: something"));
        // …while real answer text (even mentioning credentials) is kept, and blank
        // lines survive as paragraph breaks.
        assert!(!is_cli_stdout_noise("Dear Hiring Manager,"));
        assert!(!is_cli_stdout_noise(
            "I loaded cached credentials into the pipeline as described."
        ));
        assert!(!is_cli_stdout_noise(""));
    }

    #[test]
    fn arg_token_accepts_ids_and_rejects_shell_metacharacters() {
        // Real model ids / effort levels pass unchanged (trimmed).
        assert_eq!(arg_token("gpt-5-codex"), Some("gpt-5-codex"));
        assert_eq!(arg_token("gemini-2.5-pro"), Some("gemini-2.5-pro"));
        assert_eq!(arg_token("o4-mini"), Some("o4-mini"));
        assert_eq!(arg_token("high"), Some("high"));
        assert_eq!(arg_token("  gemini-2.5-flash  "), Some("gemini-2.5-flash"));
        // Shell metacharacters / whitespace-splitting / empties are rejected, so the
        // flag is omitted rather than smuggling text through `cmd.exe` on Windows
        // (the CVE-2024-24576 argv invariant, defended in depth).
        for bad in [
            "", "   ", "a b", "m&calc", "a|b", "a>b", "a<b", "a^b", "%PATH%", "a\"b", "a(b)",
            "a\r\nb", "$(x)", "`x`", "a;b", "a/b",
        ] {
            assert_eq!(arg_token(bad), None, "{bad:?} must be rejected");
        }
    }

    #[tokio::test]
    async fn cancel_poll_breaks_a_stalled_line_read() {
        use std::io;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        // Models `run_stream`'s read loop: the line read stalls forever while the
        // cancel flag flips. The `biased` select must reach the poll branch and
        // yield `ReadOutcome::Cancelled` instead of hanging on the read.
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            flag.store(true, Ordering::SeqCst);
        });

        let outcome = async {
            loop {
                tokio::select! {
                    biased;
                    // A stalled stream: the next line never arrives.
                    next = std::future::pending::<io::Result<Option<String>>>() => {
                        break match next {
                            Ok(Some(l)) => ReadOutcome::Line(l),
                            Ok(None) => ReadOutcome::Eof,
                            Err(e) => ReadOutcome::Err(e),
                        };
                    }
                    _ = tokio::time::sleep(CANCEL_POLL) => {
                        if cancelled.load(Ordering::SeqCst) {
                            break ReadOutcome::Cancelled;
                        }
                    }
                }
            }
        }
        .await;

        // Cancel observed within a bounded number of polls — no hang, and it is a
        // *cancel*, distinct from a natural EOF.
        assert!(matches!(outcome, ReadOutcome::Cancelled));
    }

    #[tokio::test]
    async fn eof_without_cancel_is_clean_completion_not_a_cancel() {
        use std::io;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        // A natural EOF that coincides with *no* cancellation must resolve to
        // `Eof` (clean break), never `Cancelled`. The biased line read wins over
        // the poll, so even if a cancel were racing the EOF still takes priority —
        // here cancel never fires, so the only correct outcome is `Eof`.
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();

        let outcome = async {
            loop {
                tokio::select! {
                    biased;
                    // Stream end: the read is immediately ready with `Ok(None)`.
                    next = async { io::Result::Ok(None::<String>) } => {
                        break match next {
                            Ok(Some(l)) => ReadOutcome::Line(l),
                            Ok(None) => ReadOutcome::Eof,
                            Err(e) => ReadOutcome::Err(e),
                        };
                    }
                    _ = tokio::time::sleep(CANCEL_POLL) => {
                        if flag.load(Ordering::SeqCst) {
                            break ReadOutcome::Cancelled;
                        }
                    }
                }
            }
        }
        .await;

        // A real EOF is never misreported as a cancellation.
        assert!(matches!(outcome, ReadOutcome::Eof));
    }

    #[tokio::test]
    async fn detect_missing_binary_is_false() {
        let (ok, version) = detect("ajh-definitely-not-a-real-binary-x9z").await;
        assert!(!ok);
        assert!(version.is_none());
    }

    #[tokio::test]
    async fn detect_cached_serves_cached_result_within_ttl() {
        let bin = "ajh-cache-probe-binary-not-real-q7w";
        // First call probes (binary missing) and caches the negative result.
        assert_eq!(detect_cached(bin).await, (false, None));
        // Poison the cache with a value a real probe could never produce, then
        // confirm the next call returns it — proving it read the cache, not the
        // binary (i.e. no re-spawn within the TTL).
        detect_cache().lock().insert(
            bin.to_string(),
            Detected {
                ok: true,
                version: Some("9.9.9".into()),
                at: Instant::now(),
            },
        );
        assert_eq!(detect_cached(bin).await, (true, Some("9.9.9".to_string())));
    }
}
