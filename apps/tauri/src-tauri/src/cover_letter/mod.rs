pub mod leakage;
pub mod research;

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::pipeline::retry::{self, DraftGenerator, RetryPolicy};
use crate::pipeline::validation::Validator;
use crate::pipeline::{Completer, Pipeline, Stage};

use leakage::LeakageValidator;
use research::CompanyResearch;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CoverLetterRequest {
    pub resume: String,
    pub job_ad: String,
    /// Generation mode forwarded to the prompt ("ats", "startup", etc.)
    pub mode: Option<String>,
    /// BCP-47 locale tag ("en", "de", …) — used in the prompt
    pub locale: Option<String>,
    /// Perform company research (default true)
    pub enable_research: Option<bool>,
    /// Run leakage validation and retry on FAIL (default true)
    pub enable_leakage_check: Option<bool>,
    /// Which provider to route through ("anthropic" | "openai" | "openai-compatible" | "ollama" | "gemini")
    pub provider: Option<String>,
    /// Provider-specific model name
    pub model: Option<String>,
    /// Base URL for openai-compatible providers
    pub base_url: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct CoverLetterResponse {
    pub text: String,
    pub company_brief: Option<String>,
    pub leakage_verdict: Option<String>,
    /// Number of regeneration attempts (0–2)
    pub retries: u8,
}

// ── Prompt constants ──────────────────────────────────────────────────────────

const COVER_LETTER_SYSTEM: &str = "\
You are an expert cover letter writer. Produce a tailored, authentic letter.

ABSOLUTE RULES — never break these:
1. Every factual claim about the candidate MUST come from <candidate_resume>.
2. You MAY reference the company name, role title, and mission from <target_job_ad>.
3. NEVER copy or closely paraphrase bullet points from the job ad.
4. NEVER claim skills, tools, certifications, or experiences not in <candidate_resume>.
5. NEVER invent metrics, dates, or outcomes.
6. Structure: hook sentence (immediate value, no \"I am excited to apply\") →
   2-3 achievement sentences backed by the resume → company-fit sentence → confident close.
   Total body: 200-300 words.
7. Bold important keywords with **double asterisks**.
8. Output the letter only — no commentary, no analysis.";

fn build_user_prompt(
    resume: &str,
    job_ad: &str,
    company_brief: &str,
    mode: &str,
    locale: &str,
) -> String {
    let brief_block = if !company_brief.is_empty() {
        format!(
            "\n<company_research>\n{company_brief}\n</company_research>\n\
             Use the company research for the fit paragraph only — \
             never as a source of candidate facts.\n"
        )
    } else {
        String::new()
    };

    format!(
        "<candidate_resume>\n{resume}\n</candidate_resume>\n\n\
         <target_job_ad>\n{job_ad}\n</target_job_ad>\n\
         {brief_block}\n\
         Mode: {mode}\n\
         Language: {locale}\n\n\
         Write the cover letter now:",
        resume = &resume[..resume.len().min(4500)],
        job_ad = &job_ad[..job_ad.len().min(2500)],
    )
}

// ── Pipeline context ────────────────────────────────────────────────────────────

/// Shared state for the cover-letter pipeline. Stages read inputs and write the
/// accumulated company/brief/draft/verdict. Provider access + event emission go
/// through the centralized [`Completer`].
struct CoverLetterContext {
    completer: Completer,
    resume: String,
    resume_summary: String,
    job_ad: String,
    mode: String,
    locale: String,
    enable_research: bool,
    enable_leakage: bool,
    // Accumulators
    company: String,
    brief: String,
    draft: String,
    leakage_verdict: Option<String>,
    retries: u8,
}

impl CoverLetterContext {
    fn new(completer: Completer, req: &CoverLetterRequest) -> Self {
        Self {
            completer,
            resume_summary: extract_resume_summary(&req.resume),
            resume: req.resume.clone(),
            job_ad: req.job_ad.clone(),
            mode: req.mode.clone().unwrap_or_else(|| "recruiter".to_string()),
            locale: req.locale.clone().unwrap_or_else(|| "en".to_string()),
            enable_research: req.enable_research.unwrap_or(true),
            enable_leakage: req.enable_leakage_check.unwrap_or(true),
            company: String::new(),
            brief: String::new(),
            draft: String::new(),
            leakage_verdict: None,
            retries: 0,
        }
    }

    fn emit(&self, event: &str, payload: Value) {
        let _ = self.completer.app().emit(event, payload);
    }

    fn into_response(self) -> CoverLetterResponse {
        CoverLetterResponse {
            text: self.draft,
            company_brief: if self.brief.is_empty() { None } else { Some(self.brief) },
            leakage_verdict: self.leakage_verdict,
            retries: self.retries,
        }
    }
}

// ── Stages ────────────────────────────────────────────────────────────────────

/// Phase 1 — company research (reusable [`crate::pipeline::enrichment::Enricher`]).
struct ResearchStage;

#[async_trait]
impl Stage<CoverLetterContext> for ResearchStage {
    fn name(&self) -> &'static str {
        "research"
    }
    async fn run(&self, ctx: &mut CoverLetterContext) -> Result<(), String> {
        if !ctx.enable_research {
            return Ok(());
        }
        use crate::pipeline::enrichment::Enricher;
        ctx.emit("cover_letter:research:start", json!({}));
        let result = CompanyResearch.enrich(&ctx.completer, &ctx.job_ad).await;
        ctx.company = result.key;
        ctx.brief = result.content;
        ctx.emit(
            "cover_letter:research:done",
            json!({ "company": ctx.company, "briefLen": ctx.brief.len() }),
        );
        Ok(())
    }
}

/// Phases 2–5 — build prompt, generate draft, validate, regenerate-if-needed,
/// via the reusable [`retry::generate_validated`] loop.
struct GenerateValidatedStage;

#[async_trait]
impl Stage<CoverLetterContext> for GenerateValidatedStage {
    fn name(&self) -> &'static str {
        "generate"
    }
    async fn run(&self, ctx: &mut CoverLetterContext) -> Result<(), String> {
        let generator = CoverLetterDraftGenerator {
            resume_summary: ctx.resume_summary.clone(),
            job_ad: ctx.job_ad.clone(),
            brief: ctx.brief.clone(),
            mode: ctx.mode.clone(),
            locale: ctx.locale.clone(),
        };
        let validator = if ctx.enable_leakage {
            Some(LeakageValidator::new(ctx.resume.clone(), ctx.job_ad.clone()))
        } else {
            None
        };
        let validator_ref = validator.as_ref().map(|v| v as &dyn Validator);

        let out = retry::generate_validated(
            &ctx.completer,
            RetryPolicy::new(3),
            &generator,
            validator_ref,
        )
        .await?;

        ctx.draft = out.text;
        ctx.leakage_verdict = out.report.map(|r| r.verdict);
        ctx.retries = out.retries;
        Ok(())
    }
}

/// Builds the cover-letter prompt and runs one non-streaming generation attempt
/// through the centralized provider. Validation needs the full text, so this is
/// `complete()` (collect), not token streaming.
struct CoverLetterDraftGenerator {
    resume_summary: String,
    job_ad: String,
    brief: String,
    mode: String,
    locale: String,
}

#[async_trait]
impl DraftGenerator for CoverLetterDraftGenerator {
    async fn generate(&self, completer: &Completer, attempt: u8) -> Result<String, String> {
        let _ = completer
            .app()
            .emit("cover_letter:generation:start", json!({ "attempt": attempt }));
        let user = build_user_prompt(&self.resume_summary, &self.job_ad, &self.brief, &self.mode, &self.locale);
        let raw = completer.complete(COVER_LETTER_SYSTEM, &user, Some(0.4)).await?;
        let text = strip_think_blocks(&raw);
        let _ = completer.app().emit(
            "cover_letter:generation:done",
            json!({ "attempt": attempt, "chars": text.len() }),
        );
        Ok(text)
    }
}

// ── Pipeline entry point (called by the Tauri command wrapper) ────────────────

pub async fn run_pipeline(
    app: AppHandle,
    req: CoverLetterRequest,
) -> Result<CoverLetterResponse, String> {
    // Resolve the active provider + model through the centralized layer.
    let completer =
        Completer::resolve(&app, req.provider.as_deref(), req.model.as_deref(), req.base_url.clone())
            .map_err(|e| {
                tracing::error!("cover_letter: provider resolution failed: {e}");
                e
            })?;

    let mut ctx = CoverLetterContext::new(completer, &req);

    Pipeline::new("cover_letter")
        .add(ResearchStage)
        .add(GenerateValidatedStage)
        .run(&mut ctx)
        .await?;

    Ok(ctx.into_response())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Lightweight resume summariser: strips markdown, truncates to 4000 chars.
fn extract_resume_summary(resume: &str) -> String {
    let cleaned = resume
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    cleaned[..cleaned.len().min(4000)].to_string()
}

/// Remove `<think>...</think>` blocks emitted by local reasoning models.
fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("<think>") {
        out.push_str(&rest[..open]);
        rest = &rest[open + 7..];
        if let Some(close) = rest.find("</think>") {
            rest = &rest[close + 8..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}
