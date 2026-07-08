//! Antigravity CLI backend — Google's `agy` CLI run headless with the prompt on stdin.
//!
//! **UNVERIFIED — implemented to the documented contract, not runtime-tested.**
//! `agy` is not installed on the build/dev machine, so this backend has never been
//! exercised end-to-end. Everything below follows Antigravity's published CLI docs
//! plus community reports; the parsing is deliberately defensive and the specifics
//! (flags, models, npm package) are provisional until a real `agy` install confirms
//! them. Mirrors [`super::gemini_cli`] because the contract is the same shape.
//!
//! **Security posture (fail-closed):** the prompt inlines untrusted, scraped
//! job-description text, so it is delivered on **stdin** ([`PromptDelivery::Stdin`]),
//! never as an argv element — nothing prompt-derived reaches the command line (or,
//! on Windows, `cmd.exe`; see the CVE-2024-24576 note on [`PromptDelivery`]). We pass
//! **no flags at all**: no `-p` (the prompt is piped), and deliberately **no `--yes`**.
//! `--yes` would auto-approve any tool action `agy` decides to take with no sandbox,
//! so a prompt-injection in the untrusted JD could trigger auto-approved side effects
//! (the reviewer's HIGH). We do not auto-approve. If `agy` blocks waiting for approval
//! without `--yes`, that surfaces as empty output at verify time — fail-closed, which
//! is acceptable for an unverified backend; RCE-by-argv and silent auto-approval are not.
//!
//! **Before this backend can be trusted**, a real `agy` install must confirm it
//! (a) consumes a piped-stdin prompt in non-interactive mode, and (b) exposes a
//! read-only / no-tools headless mode — only then is auto-approving anything safe to
//! reconsider. Until then it stays UNVERIFIED and flag-free.
//!
//! Documented contract:
//!  * Binary `agy`. Non-interactive: prompt piped on stdin (historically also
//!    accepted `agy -p "<prompt>"` / `--prompt` / `--print`, which we no longer use).
//!  * Output is **plain text on stdout** — there is no working JSON mode yet
//!    (`--output-format json` currently errors "flags provided but not defined"),
//!    so we parse plain-text stdout line-by-line like Gemini CLI, NOT JSONL like
//!    Codex. The shared plain-text hygiene ([`super::is_cli_stdout_noise`]) strips
//!    credential/telemetry noise so it never lands in the generated letter.
//!  * Auth: `agy`'s own keyring / Google sign-in — keyless here, exactly like the
//!    other CLI agents. `detect()` only checks the binary exists; no key is injected.
//!
//! KNOWN CAVEAT (upstream): under a non-TTY — which is how we always spawn it —
//! `agy` can DROP the final response line on stdout. We capture stdout fully (a
//! buffered line stream for `chat_stream`, `wait_with_output` for `complete`), which
//! is the most a caller can do from outside the tool; a dropped tail is an upstream
//! bug, not a parse bug here. Re-verify once `agy` is installed.
//!
//! Also UNVERIFIED and to confirm against a real install: a model-selection flag
//! is intentionally NOT passed (an unknown flag makes `agy` hard-error, which would
//! break every call), so `agy` currently runs its own configured/default model and
//! the [`models`](CliAgentBackend::models) list is informational for the UI only.

use async_trait::async_trait;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

/// UNVERIFIED model aliases (Antigravity is Gemini-based). Informational for the
/// UI dropdown — the selection is not forwarded on the command line yet (see the
/// module docs), so `agy` uses its own configured/default model regardless.
const MODELS: &[&str] = &["gemini-3-pro", "gemini-2.5-pro"];

pub struct AntigravityAgent;

#[async_trait]
impl CliAgentBackend for AntigravityAgent {
    fn id(&self) -> ProviderId {
        ProviderId::Antigravity
    }

    fn default_binary(&self) -> &'static str {
        "agy"
    }

    fn env_override(&self) -> &'static str {
        "ANTIGRAVITY_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    // UNVERIFIED npm package name — the one-click install runs `npm install -g` on
    // this and it must match the shell-capability allowlist entry. Correct it once
    // `agy`'s real distribution channel is confirmed (it may not be an npm package
    // at all, in which case the guide/docs path is the real install route).
    fn install_package(&self) -> &'static str {
        "@google/antigravity"
    }

    fn docs_url(&self) -> &'static str {
        "https://antigravity.google/docs"
    }

    // No system-prompt flag (like Gemini CLI) — the harness inlines the system
    // prompt onto the user prompt.
    fn inline_system(&self) -> bool {
        true
    }

    // Antigravity has no headless reasoning-effort flag — `effort` is ignored.
    fn stream_invocation(
        &self,
        _model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: headless_args(),
            // Prompt on stdin, not argv — see the security posture in the module
            // docs. No flags (no `-p`, no `--yes`); the prompt is piped.
            prompt: PromptDelivery::Stdin,
        }
    }

    fn complete_invocation(
        &self,
        _model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: headless_args(),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        // Plain-text output: emit each stdout line, preserving line breaks so
        // paragraphs survive; the harness marks completion on EOF. Drop the same
        // credential/telemetry noise Gemini emits (agy shares the Google auth stack).
        if super::is_cli_stdout_noise(line) {
            return None;
        }
        Some(CliEvent::Delta(format!("{line}\n")))
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        // Strip operational noise, rejoin (keeps the answer's paragraph breaks),
        // then trim. Captures stdout fully — see the non-TTY caveat in the module docs.
        let text = stdout
            .lines()
            .filter(|l| !super::is_cli_stdout_noise(l))
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim();
        if text.is_empty() {
            return Err(AppError::Provider(
                "Antigravity: empty response".to_string(),
            ));
        }
        Ok(text.to_string())
    }
}

/// `agy` — no flags. The prompt is piped on stdin (not `-p`), and we deliberately do
/// NOT pass `--yes`: auto-approving tool actions on an untrusted, JD-derived prompt
/// with no sandbox is the HIGH the reviewer flagged. No model flag either (see the
/// module docs). Kept as a named helper so both invocations stay in lockstep and the
/// "no `--yes`, no `-p`" invariant has one place to assert against.
fn headless_args() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_plain_text_and_filters_noise() {
        assert_eq!(
            AntigravityAgent.parse_stream_line("Hello"),
            Some(CliEvent::Delta("Hello\n".to_string()))
        );
        // Blank lines survive as paragraph breaks…
        assert_eq!(
            AntigravityAgent.parse_stream_line(""),
            Some(CliEvent::Delta("\n".to_string()))
        );
        // …credential noise does not.
        assert_eq!(
            AntigravityAgent.parse_stream_line("Loaded cached credentials."),
            None
        );
    }

    #[test]
    fn parse_complete_strips_noise_and_trims() {
        let out = "Loaded cached credentials.\nDear Team,\n\nRegards.\n";
        assert_eq!(
            AntigravityAgent.parse_complete(out).unwrap(),
            "Dear Team,\n\nRegards."
        );
        assert!(AntigravityAgent.parse_complete("   ").is_err());
    }

    #[test]
    fn argv_is_empty_prompt_on_stdin_and_never_auto_approves() {
        let inv = AntigravityAgent.stream_invocation("gemini-3-pro", "system text", None);
        // Prompt delivered on stdin — no untrusted JD text in argv / `cmd.exe`
        // (CVE-2024-24576).
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        // No prompt value flag…
        assert!(!inv.args.iter().any(|a| a == "-p"));
        // …and crucially NO `--yes`: never auto-approve tool actions on an untrusted
        // prompt (the HIGH finding). argv is empty — nothing but the piped prompt.
        assert!(!inv.args.iter().any(|a| a == "--yes"));
        assert!(inv.args.is_empty());
    }

    #[test]
    fn complete_invocation_also_never_auto_approves() {
        let inv = AntigravityAgent.complete_invocation("gemini-3-pro", "system text", None);
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        assert!(!inv.args.iter().any(|a| a == "--yes"));
        assert!(inv.args.is_empty());
    }

    #[test]
    fn inlines_system_prompt() {
        assert!(AntigravityAgent.inline_system());
    }
}
