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
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::jobs::JobTracker;
use crate::platform::NoWindow;

use super::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace, TokenParam,
};

mod claude_code;
mod codex;
mod gemini_cli;

use claude_code::ClaudeCodeAgent;
use codex::CodexAgent;
use gemini_cli::GeminiCliAgent;

/// Max wall-clock time for a single CLI generation before we kill the child.
const TIMEOUT: Duration = Duration::from_secs(300);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptDelivery {
    /// Pipe the prompt to stdin (avoids arg-length / escaping limits). Preferred.
    Stdin,
    /// Append the prompt as the **final** argument (place a value-flag like `-p`
    /// last if the agent expects the prompt as that flag's value). Constructed by
    /// the Codex / Gemini CLI backends (follow-up).
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

    /// Args for a streaming generation (model/system already resolved). `effort` is
    /// the optional reasoning effort for agents that support it (Codex); others ignore it.
    fn stream_invocation(&self, model: &str, system: &str, effort: Option<&str>) -> CliInvocation;
    /// Args for a one-shot, non-streaming generation.
    fn complete_invocation(&self, model: &str, system: &str, effort: Option<&str>) -> CliInvocation;

    /// Map one raw stdout line to a [`CliEvent`], or `None` to ignore it.
    fn parse_stream_line(&self, line: &str) -> Option<CliEvent>;
    /// Extract the final assistant text from a one-shot invocation's full stdout.
    fn parse_complete(&self, stdout: &str) -> AppResult<String>;

    /// Resolved binary path: env override, else [`default_binary`](Self::default_binary).
    fn binary(&self) -> String {
        std::env::var(self.env_override()).unwrap_or_else(|_| self.default_binary().to_string())
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

/// Whether the binary is installed (`<binary> --version` succeeds), plus its
/// reported version. Mirrors `ollama::reachable_model()` as the health signal.
pub async fn detect(binary: &str) -> (bool, Option<String>) {
    let fut = Command::new(binary).arg("--version").no_window().output();
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
        Detected { ok, version: version.clone(), at: Instant::now() },
    );
    (ok, version)
}

// ── Streaming engine ─────────────────────────────────────────────────────────────

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

    if matches!(inv.prompt, PromptDelivery::Stdin) {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes()).await;
            // `stdin` drops here → EOF, so the agent stops waiting for input.
        }
    } else {
        drop(child.stdin.take());
    }

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

    loop {
        if is_cancelled(app, job_id) {
            let _ = child.start_kill();
            trace.end(None, false);
            return Err(AppError::Message("Job cancelled".to_string()));
        }

        let line = match tokio::time::timeout_at(deadline, lines.next_line()).await {
            Err(_) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Network(format!("{label} timed out after 5 minutes.")));
            }
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => break, // clean EOF
            Ok(Err(e)) => {
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
                let _ = app.emit(
                    "ai:stream",
                    json!({ "jobId": job_id, "delta": text, "done": false }),
                );
            }
            Some(CliEvent::Thinking(text)) if !text.is_empty() => {
                let _ = app.emit(
                    "ai:stream",
                    json!({ "jobId": job_id, "delta": text, "done": false, "thinking": true }),
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

    // No explicit terminal event (e.g. plain-text agents): a clean exit or any
    // streamed text means success; otherwise surface the failure.
    if !emitted_done && !success && !any_delta {
        trace.end(status.and_then(|s| s.code()).map(|c| c as u16), false);
        return Err(friendly_cli_error(label, status.and_then(|s| s.code()), &stderr_text));
    }

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

    if matches!(inv.prompt, PromptDelivery::Stdin) {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes()).await;
        }
    } else {
        drop(child.stdin.take());
    }

    let output = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            trace.end(None, false);
            return Err(AppError::Provider(format!("{label}: {e}")));
        }
        Err(_) => {
            trace.end(None, false);
            return Err(AppError::Network(format!("{label} timed out after 5 minutes.")));
        }
    };

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

fn spawn(binary: &str, inv: &CliInvocation, prompt: &str) -> std::io::Result<tokio::process::Child> {
    let mut args = inv.args.clone();
    if matches!(inv.prompt, PromptDelivery::Arg) {
        args.push(prompt.to_string());
    }
    Command::new(binary)
        .args(&args)
        // Neutral cwd: we only want text generation, never side effects in the
        // user's project (backends also disable tools where the CLI supports it).
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_window()
        .spawn()
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
    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
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
            let label = if m.role == "assistant" { "Assistant" } else { "User" };
            format!("{label}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_includes_all_cli_agents() {
        for id in [ProviderId::ClaudeCode, ProviderId::Codex, ProviderId::GeminiCli] {
            assert!(backend_for(id).is_some(), "{} should be registered", id.as_str());
        }
        assert!(all().iter().all(|b| b.id().is_cli_agent()));
    }

    #[test]
    fn non_cli_provider_has_no_backend() {
        assert!(backend_for(ProviderId::Anthropic).is_none());
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
            Detected { ok: true, version: Some("9.9.9".into()), at: Instant::now() },
        );
        assert_eq!(detect_cached(bin).await, (true, Some("9.9.9".to_string())));
    }
}
