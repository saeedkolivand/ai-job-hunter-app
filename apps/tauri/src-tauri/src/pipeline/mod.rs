//! Reusable workflow orchestration on top of the centralized AI provider layer.
//!
//! Feature generators (resume, cover letter, future workflows) are expressed as
//! a [`Pipeline`] of modular [`Stage`]s rather than bespoke per-feature code.
//! Every stage runs on shared platform infrastructure:
//!
//! * providers / streaming / auth / capabilities — via [`Completer`] and the
//!   centralized [`crate::commands::ai_provider`] layer
//! * retries + regeneration — [`retry`]
//! * validation — [`validation`]
//! * research / enrichment — [`enrichment`]
//! * caching — [`cache`]
//! * tracing — [`StageTrace`] (per-stage) on top of the provider `RequestTrace`
//!
//! There is no feature-specific provider, auth, or request flow.

pub mod cache;
pub mod enrichment;
pub mod retry;
pub mod validation;

use std::time::Instant;

use async_trait::async_trait;
use tauri::AppHandle;

use crate::commands::ai_provider::{resolve, AiProvider, ProviderId};
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
            .ok_or_else(|| "No AI provider selected. Choose a provider in Settings → AI.".to_string())?;
        let provider_id = ProviderId::parse(provider_str)?;
        let model = model
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No model selected for the active provider.".to_string())?;
        provider_id.validate_model(model)?;
        Ok(Self {
            app: app.clone(),
            provider: resolve(provider_id, base_url),
            model: model.to_string(),
        })
    }

    /// Non-streaming completion through the active provider.
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        self.provider
            .complete(&self.app, &self.model, system, user, temperature)
            .await
    }

    /// The app handle, so stages can reach managed state (caches, credentials) and
    /// emit events without threading `AppHandle` through every signature.
    pub fn app(&self) -> &AppHandle {
        &self.app
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
        Self { name, stages: Vec::new() }
    }

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

/// Structured per-stage log, mirroring the provider `RequestTrace` style:
/// `[pipeline:cover_letter] → stage=research` / `← stage=research duration=..ms ok=true`.
struct StageTrace {
    pipeline: &'static str,
    stage: &'static str,
    start: Instant,
}

impl StageTrace {
    fn begin(pipeline: &'static str, stage: &'static str) -> Self {
        log::info!("[pipeline:{pipeline}] → stage={stage}");
        Self { pipeline, stage, start: Instant::now() }
    }

    fn end(&self, ok: bool) {
        log::info!(
            "[pipeline:{}] ← stage={} duration={}ms ok={}",
            self.pipeline,
            self.stage,
            self.start.elapsed().as_millis(),
            ok
        );
    }
}

#[cfg(test)]
mod test;
