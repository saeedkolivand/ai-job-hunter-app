//! Gemini CLI backend — Google's `gemini` CLI run headless (`gemini -p`).
//!
//! Note: distinct from the cloud Gemini API provider ([`super::super::gemini`]) —
//! this id is `gemini-cli` and shells out to the locally-installed tool, which
//! authenticates with the user's own Google / Gemini login. Output is plain text
//! on stdout (no structured event stream and no system-prompt flag), so the
//! harness inlines the system prompt and streams stdout line by line.

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
            args: prompt_args(model),
            prompt: PromptDelivery::Arg,
        }
    }

    fn complete_invocation(
        &self,
        model: &str,
        _system: &str,
        _effort: Option<&str>,
    ) -> CliInvocation {
        CliInvocation {
            args: prompt_args(model),
            prompt: PromptDelivery::Arg,
        }
    }

    fn parse_stream_line(&self, line: &str) -> Option<CliEvent> {
        // Plain-text output: emit each stdout line, preserving line breaks (blank
        // lines included, so paragraphs survive). The harness marks completion on EOF.
        Some(CliEvent::Delta(format!("{line}\n")))
    }

    fn parse_complete(&self, stdout: &str) -> AppResult<String> {
        let text = stdout.trim();
        if text.is_empty() {
            return Err(AppError::Provider("Gemini CLI: empty response".to_string()));
        }
        Ok(text.to_string())
    }
}

/// `gemini [-m <model>] -p` — the harness appends the prompt as the final arg, so
/// it lands as the value of `-p`.
fn prompt_args(model: &str) -> Vec<String> {
    let mut args = Vec::new();
    if !model.trim().is_empty() {
        args.push("-m".to_string());
        args.push(model.to_string());
    }
    args.push("-p".to_string());
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
    fn prompt_args_place_p_last_for_appended_prompt() {
        let inv = GeminiCliAgent.stream_invocation("gemini-2.5-flash", "", None);
        assert_eq!(inv.prompt, PromptDelivery::Arg);
        assert_eq!(inv.args.last().unwrap(), "-p");
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "-m" && w[1] == "gemini-2.5-flash"));
    }
}
