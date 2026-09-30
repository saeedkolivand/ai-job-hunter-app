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
//!
//! Split (R8): the contract lives in [`backend`], detection in [`detect`],
//! process launch in [`spawn`], CLI-error translation in [`errors`], the
//! `AiProvider` adapter in [`client`], and the streaming/one-shot engines in
//! [`stream`]/[`complete`]. Each backend is still its own module.

use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

#[cfg(test)]
use serde_json::{json, Value};

#[cfg(test)]
use crate::error::AppError;

#[cfg(test)]
use super::AiProvider;
use super::{ProviderId, RequestTrace};

mod antigravity;
mod backend;
mod claude_code;
mod client;
mod codex;
mod complete;
mod cursor;
mod detect;
mod errors;
mod gemini_cli;
mod opencode;
mod qwen_code;
mod spawn;
mod stream;
mod workspace;

use antigravity::AntigravityAgent;
use backend::{
    effective_prompt, is_cli_stdout_noise, text_blocks, CliAgentBackend, CliEvent, CliInvocation,
    PromptDelivery,
};
use claude_code::ClaudeCodeAgent;
#[cfg(test)]
use client::resolve_models;
pub use client::CliAgentClient;
use codex::CodexAgent;
use cursor::CursorAgent;
use detect::cli_command;
pub use detect::{clear_detect_cache, detect_cached};
#[cfg(test)]
use detect::{detect, detect_cache, Detected};
use errors::{friendly_cli_error, spawn_error, terminal_error};
use gemini_cli::GeminiCliAgent;
use opencode::OpencodeAgent;
use qwen_code::QwenCodeAgent;
use spawn::{arg_token, spawn, write_prompt_stdin};
#[cfg(test)]
use stream::{ReadOutcome, CANCEL_POLL};
use workspace::{prepare_workspace, Workspace};

/// Max wall-clock time for a single CLI generation before we kill the child.
const TIMEOUT: Duration = Duration::from_secs(300);

// ── Registry (single source of truth) ───────────────────────────────────────────

/// Every registered CLI agent. Routing and detection both read this — adding an
/// agent here is all that's needed to surface it.
pub fn all() -> Vec<Box<dyn CliAgentBackend>> {
    vec![
        Box::new(ClaudeCodeAgent),
        Box::new(CodexAgent),
        Box::new(GeminiCliAgent),
        Box::new(AntigravityAgent),
        Box::new(OpencodeAgent),
        Box::new(CursorAgent),
        Box::new(QwenCodeAgent),
    ]
}

/// The backend for a provider id, if it is a CLI agent.
pub fn backend_for(id: ProviderId) -> Option<Box<dyn CliAgentBackend>> {
    all().into_iter().find(|b| b.id() == id)
}

#[cfg(test)]
mod tests;
