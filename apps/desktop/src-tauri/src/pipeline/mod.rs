//! Reusable workflow orchestration on top of the centralized AI provider layer.
//!
//! Feature generators (resume, cover letter, future workflows) are expressed as
//! a [`Pipeline`] of modular [`Stage`]s rather than bespoke per-feature code.
//! Every stage runs on shared platform infrastructure:
//!
//! * providers / streaming / auth / capabilities — via [`Completer`] and the
//!   centralized [`crate::commands::ai_provider`] layer
//! * research / enrichment — [`enrichment`]
//! * caching — [`cache`]
//! * tracing — [`StageTrace`] (per-stage) on top of the provider `RequestTrace`
//!
//! There is no feature-specific provider, auth, or request flow.

pub mod cache;
pub mod enrichment;

use async_trait::async_trait;
use tauri::AppHandle;

use crate::commands::ai_provider::{
    record_usage, resolve, AgentTurn, AiProvider, ChatMsg, ModelCapabilities, ProviderId, ToolSpec,
};
use crate::error::AppResult;

// ── Completer ───────────────────────────────────────────────────────────────────

/// Binds the active provider + model + app handle so any pipeline stage can run a
/// non-streaming completion through the *centralized* provider layer — same
/// `resolve()`, keychain auth, capabilities, and request tracing as chat. Shared
/// platform infrastructure, not a per-feature detail.
pub struct Completer {
    app: AppHandle,
    provider: Box<dyn AiProvider>,
    model: String,
    /// The resolved base URL, when one was supplied — only meaningful for
    /// `openai-compatible` (LM Studio/vLLM/OpenRouter/…). Threaded through to
    /// [`record_usage`]'s free/paid cost gate; `None` for every other
    /// provider, which the gate ignores it for.
    base_url: Option<String>,
}

impl Completer {
    /// Resolve a request's provider/model into a `Completer`. The provider is
    /// **required and validated** — unknown/missing providers and model/provider
    /// mismatches are hard errors, never a silent fallback.
    pub fn resolve(
        app: &AppHandle,
        provider: Option<&str>,
        model: Option<&str>,
        base_url: Option<String>,
    ) -> AppResult<Self> {
        let provider_str = provider
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "No AI provider selected. Choose a provider in Settings → AI.".to_string()
            })?;
        let provider_id = ProviderId::parse(provider_str)?;
        let model = match model.map(str::trim).filter(|s| !s.is_empty()) {
            Some(m) => m.to_string(),
            // CLI agents may run with no explicit model — they fall back to the
            // tool's own configured default (validated leniently below).
            None if provider_id.is_cli_agent() => String::new(),
            None => {
                return Err("No model selected for the active provider."
                    .to_string()
                    .into())
            }
        };
        provider_id.validate_model(&model)?;
        Ok(Self {
            app: app.clone(),
            provider: resolve(provider_id, base_url.clone()),
            model,
            base_url,
        })
    }

    /// Company-research brief through the active provider's **own** web search.
    /// Returns `""` (never an error) when the provider can't search — see
    /// [`AiProvider::research`](crate::commands::ai_provider::AiProvider::research).
    pub async fn research(&self, company: &str, role: &str) -> AppResult<String> {
        self.provider
            .research(&self.app, &self.model, company, role)
            .await
    }

    /// Web-grounded market salary-range lookup through the active provider's
    /// **own** web search. Returns raw (possibly noisy) text — `""` when the
    /// provider can't search — see
    /// [`AiProvider::research_salary`](crate::commands::ai_provider::AiProvider::research_salary).
    /// The caller ([`crate::salary_research::SalaryResearch`]) parses + strictly
    /// validates it before anything reaches a prompt. `country`/`currency`
    /// ground the report in the job's actual currency; both empty when unknown.
    pub async fn research_salary(
        &self,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        self.provider
            .research_salary(
                &self.app,
                &self.model,
                role,
                company,
                location,
                country,
                currency,
            )
            .await
    }

    /// Web-search reference notes for a single application-question answer
    /// through the active provider's **own** web search — the per-question
    /// sibling of [`research`](Self::research). Returns `""` (never an error)
    /// when the provider can't search — see
    /// [`AiProvider::research_answer`](crate::commands::ai_provider::AiProvider::research_answer).
    pub async fn research_answer(
        &self,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        self.provider
            .research_answer(&self.app, &self.model, question, role, company)
            .await
    }

    /// The app handle, so stages can reach managed state (caches, credentials) and
    /// emit events without threading `AppHandle` through every signature.
    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    /// The resolved provider's id — e.g. so a caller can charge the shared
    /// per-provider daily budget ([`crate::limits::Limiter::charge_provider_daily`])
    /// after resolving, without re-parsing the provider string itself.
    pub fn provider_id(&self) -> ProviderId {
        self.provider.id()
    }

    /// The active model's capability matrix (e.g. `supports_tools`) — used by
    /// callers that must gate behavior before making a request. The agent
    /// controller requires native tool-calling; see
    /// [`crate::commands::agent::agent_run`].
    pub fn capabilities(&self) -> ModelCapabilities {
        self.provider.capabilities(&self.model)
    }

    /// Non-streaming completion through the active provider — the single-shot text
    /// analogue used by agentic text-generating tools (cover letter, interview
    /// questions) that need the whole response before returning. Reuses the same
    /// resolved provider + keychain auth + tracing as chat.
    ///
    /// This is the shared non-streaming-text chokepoint for AI-spend visibility
    /// (`crate::spend`): every call records the provider's REAL reported token
    /// usage (zero when a provider genuinely reports none) against today's
    /// spend before returning — covering autopilot notes and the résumé/cover
    /// pipeline, with zero changes needed at either call site. The agent
    /// controller's multi-turn tool-calling runs through
    /// [`chat_with_tools`](Self::chat_with_tools) instead, which records its
    /// own usage the same way.
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        let (text, usage) = self
            .provider
            .complete_with_usage(&self.app, &self.model, system, user, temperature)
            .await?;
        record_usage(
            &self.app,
            self.provider.id().as_str(),
            &self.model,
            usage.input_tokens,
            usage.output_tokens,
            self.base_url.as_deref(),
        );
        Ok(text)
    }

    /// One agentic tool-calling turn through the active provider — the multi-turn
    /// analogue of [`research`](Self::research). Delegates to
    /// [`AiProvider::chat_with_tools`](crate::commands::ai_provider::AiProvider::chat_with_tools):
    /// providers without native tool support degrade to a single-shot answer.
    /// Consumed by the agent controller (`crate::agent`) — plausibly the
    /// biggest paid-token consumer, since one "Prep this application" run fans
    /// out into several turns. Records the returned [`AgentTurn::usage`]
    /// (each provider's own turn-parser populates it from the same
    /// response fields `complete`/`chat_stream` already parse) against
    /// today's AI spend before returning, so every turn — not just the final
    /// one — is counted.
    pub async fn chat_with_tools(
        &self,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        let turn = self
            .provider
            .chat_with_tools(&self.app, &self.model, messages, tools, temperature)
            .await?;
        record_usage(
            &self.app,
            self.provider.id().as_str(),
            &self.model,
            turn.usage.input_tokens,
            turn.usage.output_tokens,
            self.base_url.as_deref(),
        );
        Ok(turn)
    }
}

// ── Stage / Pipeline ──────────────────────────────────────────────────────────────

/// One modular step of a workflow, operating on a shared mutable context `C`.
#[async_trait]
pub trait Stage<C>: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, ctx: &mut C) -> AppResult<()>;
}

/// An ordered sequence of [`Stage`]s sharing a context. Runs each stage in order,
/// emitting a per-stage [`StageTrace`]; the first error aborts the pipeline.
pub struct Pipeline<C> {
    name: &'static str,
    stages: Vec<Box<dyn Stage<C>>>,
}

impl<C> Pipeline<C> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            stages: Vec::new(),
        }
    }

    // Fluent builder verb (`Pipeline::new(..).add(stage).add(stage)`), not
    // arithmetic. Surfaced by clippy only once this became a library crate (a
    // bin crate has no public API for the lint to inspect). See `benches/`.
    #[allow(clippy::should_implement_trait)]
    pub fn add<S: Stage<C> + 'static>(mut self, stage: S) -> Self {
        self.stages.push(Box::new(stage));
        self
    }

    pub async fn run(&self, ctx: &mut C) -> AppResult<()> {
        for stage in &self.stages {
            let trace = StageTrace::begin(self.name, stage.name());
            match stage.run(ctx).await {
                Ok(()) => trace.end(true),
                Err(e) => {
                    trace.end(false);
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

// ── Stage tracing ───────────────────────────────────────────────────────────────

/// Structured per-stage log over the shared [`crate::observability::Span`]:
/// `[pipeline:cover_letter] → stage=research` / `← stage=research duration=..ms ok=true`.
struct StageTrace {
    span: crate::observability::Span,
}

impl StageTrace {
    fn begin(pipeline: &'static str, stage: &'static str) -> Self {
        Self {
            span: crate::observability::Span::begin(
                format!("pipeline:{pipeline}"),
                format!("stage={stage}"),
            ),
        }
    }

    fn end(&self, ok: bool) {
        self.span.end(ok);
    }
}

#[cfg(test)]
mod test;
