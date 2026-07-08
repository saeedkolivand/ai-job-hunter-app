//! Gemini CLI backend — Google's `gemini` CLI run headless with the prompt on stdin.
//!
//! Note: distinct from the cloud Gemini API provider ([`super::super::gemini`]) —
//! this id is `gemini-cli` and shells out to the locally-installed tool, which
//! authenticates with the user's own Google / Gemini login. Output is plain text
//! on stdout (no structured event stream and no system-prompt flag), so the
//! harness inlines the system prompt and streams stdout line by line.
//!
//! The (untrusted, JD-bearing) prompt is delivered on **stdin**
//! ([`PromptDelivery::Stdin`]): with a piped, non-TTY stdin `gemini` runs
//! non-interactively and treats the piped text as the prompt — so we pass NO `-p`
//! and nothing prompt-derived ever reaches argv (or, on Windows, `cmd.exe` — see the
//! CVE-2024-24576 note on [`PromptDelivery`]). argv holds only the optional model flag.

use async_trait::async_trait;

use crate::commands::ai_provider::ProviderId;
use crate::error::{AppError, AppResult};

use super::{CliAgentBackend, CliEvent, CliInvocation, PromptDelivery};

const MODELS: &[&str] = &["gemini-2.5-pro", "gemini-2.5-flash"];

pub struct GeminiCliAgent;

#[async_trait]
impl CliAgentBackend for GeminiCliAgent {
    fn id(&self) -> ProviderId {
        ProviderId::GeminiCli
    }

    fn default_binary(&self) -> &'static str {
        "gemini"
    }

    fn env_override(&self) -> &'static str {
        "GEMINI_CLI_BIN"
    }

    fn models(&self) -> &'static [&'static str] {
        MODELS
    }

    fn install_package(&self) -> &'static str {
        "@google/gemini-cli"
    }

    fn docs_url(&self) -> &'static str {
        "https://geminicli.com/docs/get-started/installation/"
    }

    fn inline_system(&self) -> bool {
        true
    }

    // Gemini CLI has no headless reasoning-effort flag — `effort` is ignored.
    fn stream_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: model_args(model),
            // Prompt on stdin, not argv — piped stdin is read as the prompt in
            // non-interactive mode. Keeps untrusted JD text off the command line.
            prompt: PromptDelivery::Stdin,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: model_args(model),
            prompt: PromptDelivery::Stdin,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        // Plain-text output: emit each stdout line, preserving line breaks (blank
        // lines included, so paragraphs survive). The harness marks completion on
        // EOF. Gemini prints credential/telemetry notices ("Loaded cached
        // credentials.", …) to stdout around the answer — drop them so they don't
        // get folded into the generated letter.
        if super::is_cli_stdout_noise(line) {
            return None;
        }
        Some(CliEvent::Delta(format!("{line}\n")))
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        // Same hygiene as the streaming path: strip the operational noise lines,
        // then trim. Rejoining with `\n` keeps the answer's own paragraph breaks.
        let text = stdout
            .lines()
            .filter(|l| !super::is_cli_stdout_noise(l))
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim();
        if text.is_empty() {
            return Err(AppError::Provider("Gemini CLI: empty response".to_string()));
        }
        Ok(text.to_string())
    }
}

/// `gemini [-m <model>]` — the prompt is piped on stdin (non-interactive mode), so
/// there is no `-p` value flag: argv is only the optional, trusted model selector.
fn model_args(model: &str) -> Vec<String> {
    let mut args = Vec::new();
    if !model.trim().is_empty() {
        args.push("-m".to_string());
        args.push(model.to_string());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_plain_text_lines_with_breaks() {
        assert_eq!(
            GeminiCliAgent.parse_stream_line("Hello"),
            Some(CliEvent::Delta("Hello\n".to_string()))
        );
        // Blank lines are preserved as paragraph breaks.
        assert_eq!(
            GeminiCliAgent.parse_stream_line(""),
            Some(CliEvent::Delta("\n".to_string()))
        );
    }

    #[test]
    fn parse_complete_trims_output() {
        assert_eq!(
            GeminiCliAgent.parse_complete("  the answer\n\n").unwrap(),
            "the answer"
        );
        assert!(GeminiCliAgent.parse_complete("   ").is_err());
    }

    #[test]
    fn credential_noise_is_filtered_from_both_paths() {
        // The streaming path drops the notice line but keeps the answer line.
        assert_eq!(
            GeminiCliAgent.parse_stream_line("Loaded cached credentials."),
            None
        );
        assert_eq!(
            GeminiCliAgent.parse_stream_line("Dear Team,"),
            Some(CliEvent::Delta("Dear Team,\n".to_string()))
        );
        // The one-shot path strips the notice and returns only the answer body.
        let out = "Loaded cached credentials.\nDear Team,\n\nThanks.\n";
        assert_eq!(
            GeminiCliAgent.parse_complete(out).unwrap(),
            "Dear Team,\n\nThanks."
        );
    }

    #[test]
    fn argv_has_model_only_no_prompt_flag_prompt_on_stdin() {
        let inv = GeminiCliAgent.stream_invocation("gemini-2.5-flash", "system text", None);
        // Prompt delivered on stdin — the CVE-2024-24576 fix: no untrusted JD text
        // in argv / `cmd.exe`.
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
        // The `-p` value flag is gone; argv is exactly the trusted model selector.
        assert!(!inv.args.iter().any(|a| a == "-p"));
        assert_eq!(inv.args, vec!["-m", "gemini-2.5-flash"]);
    }

    #[test]
    fn argv_is_empty_when_no_model() {
        // No model → no flags at all; the entire prompt still goes on stdin.
        let inv = GeminiCliAgent.stream_invocation("", "", None);
        assert!(inv.args.is_empty());
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
    }
}
