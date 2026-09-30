//! Adapts any [`CliAgentBackend`] to the centralized [`AiProvider`] trait —
//! the CLI-agent half of provider dispatch, plus the model-discovery
//! fallback and the request→prompt text helpers it needs.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai_provider::{
    research, structured, AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, TokenParam,
    Usage,
};
use crate::error::{AppError, AppResult};

use super::claude_code;
use super::complete::{run_complete, run_structured_complete};
use super::detect::detect;
use super::stream::run_stream;
use super::CliAgentBackend;

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
            // Every CLI coding agent reasons internally regardless of
            // backend — `true` uniformly. This is DELIBERATELY not mirrored
            // by `effort_levels()` below, which is empty for every backend
            // except Codex and Claude Code (the app has no lever into the
            // others' effort, even though they still reason) — see
            // `AiProvider::effort_levels`'s doc comment for the full
            // distinction.
            supports_reasoning: true,
            supports_tools: false,
            supports_json_mode: false,
            supports_embeddings: false,
            // The CLI agent carries its own web tools in headless mode.
            supports_web_search: true,
            token_param: TokenParam::MaxTokens,
        }
    }

    /// Only Codex (`codex::exec_args`'s `-c model_reasoning_effort=…` override)
    /// and Claude Code (`claude_code::push_effort`'s `--effort` flag) actually
    /// read `effort` — every other CLI agent's `stream_invocation`/
    /// `complete_invocation` accepts the `effort` parameter but ignores it, so
    /// the picker must not appear for them.
    fn effort_levels(&self, _model: &str) -> Vec<&'static str> {
        match self.backend.id() {
            ProviderId::Codex => vec!["low", "medium", "high"],
            ProviderId::ClaudeCode => claude_code::EFFORT_LEVELS.to_vec(),
            _ => Vec::new(),
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

    async fn complete_structured(
        &self,
        app: &AppHandle,
        req: &AiGenerateRequest,
        schema_hint: &str,
        schema: Option<&Value>,
    ) -> AppResult<(String, Usage)> {
        // No schema → nothing to constrain the CLI decoding with: the shared
        // prompt-discipline fallback, byte-identical to the trait default.
        let Some(schema) = schema else {
            return structured::prompt_only(self, app, req, schema_hint).await;
        };
        // The filled example rides in the prompt on the native path too —
        // exactly like `structured.rs`'s HTTP providers (the CLI constrains
        // decoding, the prompt still describes the shape).
        let (system, user) = structured::structured_prompt(req, schema_hint);
        let Some(inv) = self.backend.native_json_schema_invocation(
            &req.model,
            &system,
            req.effort.as_deref(),
            schema,
        ) else {
            // Backend has no native constrained-output invocation (or the
            // schema exceeded its argv cap) — unchanged fallback.
            return structured::prompt_only(self, app, req, schema_hint).await;
        };
        let text =
            run_structured_complete(app, self.backend.as_ref(), &req.model, &system, &user, inv)
                .await?;
        // Same spend contract as every other CLI path: the agent reports no
        // usage, `Usage::default()` is honest, and the caller
        // (`pipeline::Completer`) records spend — never `record_usage` here
        // (see the `(String, Usage)` signature on `prompt_only`).
        Ok((text, Usage::default()))
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

    async fn list_models(&self, _app: &AppHandle) -> AppResult<Vec<Value>> {
        // Still genuinely infallible (unlike every HTTP-backed provider): a
        // backend's `discover_models` degrades to `None` on any failure rather
        // than propagating one, so there is always a list to return — see
        // `resolve_models`'s doc comment for the fallback + labelling rule.
        Ok(resolve_models(
            self.backend.discover_models().await,
            self.backend.models(),
        ))
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

/// Choose between a backend's live [`CliAgentBackend::discover_models`]
/// result and its curated [`CliAgentBackend::models`] fallback — pure, so
/// it's covered without spawning a CLI or a mock `AppHandle` (this crate has
/// neither; see `stream.rs`'s doc comment on `finish`). An empty `Some`
/// (discovery ran but found nothing usable) counts the same as `None` —
/// either way there's nothing live to show.
pub(super) fn resolve_models(discovered: Option<Vec<Value>>, fallback: &[&str]) -> Vec<Value> {
    match discovered {
        Some(models) if !models.is_empty() => models,
        _ => fallback_models(fallback),
    }
}

/// The curated alias list, each entry explicitly labelled `source: "fallback"` so a
/// stale hardcoded list can never masquerade as the CLI's own live catalogue (#1185).
fn fallback_models(aliases: &[&str]) -> Vec<Value> {
    aliases
        .iter()
        .map(|m| json!({ "name": m, "source": "fallback" }))
        .collect()
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
