//! The CLI-agent contract: the neutral event/invocation types plus the
//! [`CliAgentBackend`] trait every backend implements, and the small
//! output-parsing helpers backends share.

use async_trait::async_trait;
use serde_json::Value;

use crate::commands::ai_provider::ProviderId;
use crate::error::AppResult;

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
/// `cmd.exe /C` ([`super::cli_command`]). Rust's batch-argument escaping (the CVE
/// fix) only engages when the spawned program is the `.cmd` itself — here the
/// program is `cmd.exe`, not the `.cmd`, so it does NOT engage. A prompt
/// containing `" & <cmd>` would then break out of `cmd.exe`'s parser and execute.
/// The structural fix: **untrusted text must never transit argv.** Every backend
/// uses [`Stdin`](Self::Stdin), so the command line carries only fixed, trusted
/// flags and the harness pipes the prompt to `child.stdin`. Fail-closed — a CLI
/// that ignores stdin yields empty output, never RCE.
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
    /// Model aliases offered in the UI (e.g. `["sonnet", "opus", "haiku", "fable"]`) —
    /// the LAST-RESORT fallback used only when [`discover_models`](Self::discover_models)
    /// has no live source or its attempt fails; never the primary source of truth for
    /// a backend that can enumerate its own models.
    fn models(&self) -> &'static [&'static str];

    /// Live model discovery for a CLI that can enumerate its own catalogue (e.g.
    /// Codex's `codex debug models`) — `None` (the default) means "no such source",
    /// so every backend but Codex falls straight through to
    /// [`models`](Self::models) unchanged. A `Some` with no usable entries counts the
    /// same as `None`. See [`super::CliAgentClient::list_models`] for the fallback +
    /// labelling.
    async fn discover_models(&self) -> Option<Vec<Value>> {
        None
    }

    /// The npm package that provides this agent's binary, for the in-app install
    /// (#22). The one-click install runs `npm install -g <this>` — and that exact
    /// command MUST also be present in the shell capability allowlist
    /// (`capabilities/default.json`); a test asserts the two agree.
    /// Returns `None` if the agent is not distributed via npm (no one-click install,
    /// only the docs/guide path).
    fn install_package(&self) -> Option<&'static str>;

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

    /// Native constrained-output invocation for this backend, if it can decode
    /// a JSON Schema at the CLI level: `Some(invocation)` routes
    /// [`super::AiProvider::complete_structured`]'s native path; `None` (the
    /// default — every backend but Claude Code) keeps the shared
    /// prompt-discipline fallback (`structured::prompt_only`) byte-identical.
    /// Backends that do opt in must bound the schema's argv length themselves
    /// (it rides argv, even though it comes from OUR `json!` literals, not
    /// user input) and must forward `effort` — it reaches this path exactly
    /// like the stream/complete invocations.
    fn native_json_schema_invocation(
        &self,
        _model: &str,
        _system: &str,
        _effort: Option<&str>,
        _schema: &Value,
    ) -> Option<CliInvocation> {
        None
    }

    /// Parse a native structured invocation's full stdout. DEFAULT:
    /// [`parse_complete`](Self::parse_complete) — a backend whose native path
    /// is "the same output, constrained" needs nothing more. Claude Code
    /// overrides this to read its schema-validated `structured_output` field.
    fn parse_structured_complete(&self, stdout: &str) -> AppResult<String> {
        self.parse_complete(stdout)
    }

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

    /// Config files (e.g. the tool-refusing config) written before every spawn, as
    /// `(relative_path, contents)`. The harness writes them into a fresh per-spawn
    /// directory (see `workspace.rs`) and uses it as `current_dir`. Default: no
    /// files, and the CLI runs in `temp_dir()`.
    fn workspace_files(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    /// Environment variables that point the CLI at one of its
    /// [`workspace_files`](Self::workspace_files), as `(variable, relative_path)`;
    /// the harness sets each to that file's absolute path in the spawn's
    /// workspace. For config a CLI only honours from a fixed scope (Qwen's
    /// system settings). Default: none.
    fn workspace_env(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }
}

/// The text of every `{"type":"text","text":…}` block in a message's `content`
/// array, in order. Shared by the Cursor and Qwen parsers (same message shape).
pub(super) fn text_blocks(content: &[Value]) -> String {
    content
        .iter()
        .filter(|c| c.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
        .collect()
}

/// Combine system + user per the backend's [`inline_system`](CliAgentBackend::inline_system).
pub(super) fn effective_prompt(
    backend: &dyn CliAgentBackend,
    system: &str,
    prompt: &str,
) -> String {
    if backend.inline_system() && !system.trim().is_empty() {
        format!("{system}\n\n{prompt}")
    } else {
        prompt.to_string()
    }
}

/// Plain-text CLI agents (Gemini CLI, Antigravity) interleave the model's answer
/// on stdout with a few operational lines — cached-credential notices, telemetry
/// banners, dotenv/deprecation logs. Left in, they get folded into the generated
/// letter. This recognizes those specific, exact-ish markers so a backend can drop
/// them. Deliberately conservative — it matches only known operational lines (full
/// exact matches or unmistakable log prefixes), never anything that could be real
/// answer text, and treats blank lines as paragraph breaks (kept).
pub(super) fn is_cli_stdout_noise(line: &str) -> bool {
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
