//! Provider identity — the canonical enum and its associated methods.
//!
//! This module was split out of `mod.rs` to keep that file under the R8 1400-LOC hard cap.

use crate::commands::ai_provider::cli_agent;
use crate::error::{AppError, AppResult};

/// Every supported provider. Stringly-typed provider checks are banned in favor
/// of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    Ollama,
    /// Ollama Cloud — hosted Ollama models over its OpenAI-compatible endpoint
    /// (`ollama.com/v1`). Chat reuses the OpenAI client; the same account key
    /// (`ai:ollama-cloud`) also powers Ollama Web Search for company research.
    OllamaCloud,
    OpenAi,
    /// Any OpenAI-compatible server (LM Studio, vLLM, OpenRouter, Groq,
    /// Together, DeepSeek, Azure-style gateways…) addressed via a custom base URL.
    OpenAiCompatible,
    Anthropic,
    Gemini,
    /// Anthropic Claude Code CLI run headless (a [`cli_agent`] backend). Local +
    /// keyless: authenticates with the user's own Claude Code login.
    ClaudeCode,
    /// OpenAI Codex CLI run headless (a [`cli_agent`] backend). Keyless: uses the
    /// user's ChatGPT login or `OPENAI_API_KEY`.
    Codex,
    /// Google Gemini CLI run headless (a [`cli_agent`] backend) — distinct from the
    /// cloud [`Gemini`](Self::Gemini) API. Keyless: uses the user's Google login.
    GeminiCli,
    /// Google Antigravity CLI (`agy`) run headless (a [`cli_agent`] backend).
    /// Keyless: uses `agy`'s own Google sign-in. **UNVERIFIED** — implemented to
    /// the documented CLI contract but not runtime-tested (see `cli_agent::antigravity`).
    Antigravity,
    /// opencode CLI run headless (a [`cli_agent`] backend). Keyless: uses the
    /// user's own opencode login.
    Opencode,
    /// Cursor CLI (`cursor-agent`) run headless (a [`cli_agent`] backend). Keyless:
    /// uses the user's Cursor login.
    Cursor,
    /// Qwen Code CLI run headless (a [`cli_agent`] backend). Keyless: uses the
    /// user's Qwen login.
    QwenCode,
    // Copilot removed — JSONL output format undocumented, invocation was wrong,
    // deny list incomplete. Re-add in a future PR when real output can be captured.
}

impl ProviderId {
    /// Parse a wire string. Unknown values are a hard error — never a fallback.
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "ollama" => Ok(Self::Ollama),
            "ollama-cloud" => Ok(Self::OllamaCloud),
            "openai" => Ok(Self::OpenAi),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            "claude-code" => Ok(Self::ClaudeCode),
            "codex" => Ok(Self::Codex),
            "gemini-cli" => Ok(Self::GeminiCli),
            "antigravity" => Ok(Self::Antigravity),
            "opencode" => Ok(Self::Opencode),
            "cursor" => Ok(Self::Cursor),
            "qwen-code" => Ok(Self::QwenCode),
            // "copilot" => Ok(Self::Copilot), // REMOVED — see Copilot variant comment
            other => Err(AppError::Config(format!(
                "Unknown AI provider '{other}'. Select a configured provider in Settings → AI."
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OllamaCloud => "ollama-cloud",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::GeminiCli => "gemini-cli",
            Self::Antigravity => "antigravity",
            Self::Opencode => "opencode",
            Self::Cursor => "cursor",
            Self::QwenCode => "qwen-code",
            // Copilot removed — deny list incomplete. Re-add in a future PR when real output can be captured.
        }
    }

    /// Credential-store key suffix (`ai:<key>`). Ollama needs none.
    pub fn credential_key(&self) -> &'static str {
        self.as_str()
    }

    /// Whether this provider runs locally (no API key, no outbound cloud call):
    /// the Ollama server or any CLI agent.
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Ollama) || self.is_cli_agent()
    }

    /// Whether this provider is a headless CLI agent (Claude Code, …).
    pub fn is_cli_agent(&self) -> bool {
        cli_agent::backend_for(*self).is_some()
    }

    /// Guard against picking a model that clearly belongs to a *different*
    /// provider (a likely UI mistake). Deliberately permissive otherwise:
    /// **unknown / newly-released model names are allowed**, so the app adopts new
    /// models with no code change. Ollama, OpenAI-compatible (OpenRouter serves
    /// `anthropic/…` and `google/…` models!), and CLI agents accept any name.
    pub fn validate_model(&self, model: &str) -> AppResult<()> {
        let m = model.trim().to_ascii_lowercase();
        if m.is_empty() {
            // CLI agents fall back to the tool's own configured default model.
            if self.is_cli_agent() {
                return Ok(());
            }
            return Err(AppError::Config(
                "No model selected for the active provider.".to_string(),
            ));
        }
        let looks_anthropic = m.starts_with("claude");
        let looks_gemini = m.starts_with("gemini") || m.starts_with("models/gemini");
        let looks_openai = m.starts_with("gpt")
            || m.starts_with("chatgpt")
            || m.starts_with("o1")
            || m.starts_with("o3")
            || m.starts_with("o4");

        let mismatch = || {
            Err(AppError::Validation(format!(
                "Model '{model}' looks like another provider's model, but the active provider is {}. \
                 Pick a matching model or switch providers.",
                self.as_str()
            )))
        };

        // Only reject a model that unambiguously belongs to a *different* native
        // cloud family — never reject a merely-unrecognized name, so new releases
        // work without a code change.
        match self {
            Self::Anthropic if looks_openai || looks_gemini => mismatch(),
            Self::Gemini if looks_anthropic || looks_openai => mismatch(),
            Self::OpenAi if looks_anthropic || looks_gemini => mismatch(),
            _ => Ok(()),
        }
    }
}
