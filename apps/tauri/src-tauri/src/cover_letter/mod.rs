pub mod cache;
pub mod leakage;
pub mod research;

use parking_lot::Mutex;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use cache::CompanyBriefCache;

use crate::commands::ai_provider::{resolve, AiProvider, ProviderId};

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
    /// Which LLM provider to use ("anthropic" | "openai" | "openai-compatible" | "ollama")
    pub provider: Option<String>,
    /// Provider-specific model name override
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

// ── Centralized completion binding ──────────────────────────────────────────────

/// Binds the active provider + model + app handle so the pipeline's passes
/// (generation, company-brief synthesis, leakage validation) can call a simple
/// `complete(system, user)` while routing through the *centralized* provider
/// layer — same `resolve()`, auth, capabilities, and request tracing as chat.
/// There is no separate "fast" model tier: ancillary passes reuse the selected
/// model so no provider-specific model names are hard-coded here.
pub struct Completer {
    app: AppHandle,
    provider: Box<dyn AiProvider>,
    model: String,
}

impl Completer {
    pub async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        self.provider
            .complete(&self.app, &self.model, system, user, Some(0.4))
            .await
    }
}

/// Resolve the request's provider/model into a `Completer`.
/// The provider is **required and validated** — unknown/missing providers and
/// model/provider mismatches are hard errors, never a silent Ollama fallback.
/// API keys are resolved inside the provider client from the OS keychain.
fn resolve_completer(app: &AppHandle, req: &CoverLetterRequest) -> Result<Completer, String> {
    let provider_str = req
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "No AI provider selected. Choose a provider in Settings → AI.".to_string())?;
    let provider_id = ProviderId::parse(provider_str)?;
    let model = req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "No model selected for the active provider.".to_string())?;
    provider_id.validate_model(model)?;

    Ok(Completer {
        app: app.clone(),
        provider: resolve(provider_id, req.base_url.clone()),
        model: model.to_string(),
    })
}

// ── Emit helpers ──────────────────────────────────────────────────────────────

fn emit(app: &AppHandle, event: &str, payload: serde_json::Value) {
    if let Err(e) = app.emit(event, payload) {
        tracing::warn!("emit {event} failed: {e}");
    }
}

// ── Pipeline entry point (called by the Tauri command wrapper) ────────────────

pub async fn run_pipeline(
    app: AppHandle,
    req: CoverLetterRequest,
) -> Result<CoverLetterResponse, String> {
    let enable_research = req.enable_research.unwrap_or(true);
    let enable_leakage = req.enable_leakage_check.unwrap_or(true);
    let mode = req.mode.as_deref().unwrap_or("recruiter");
    let locale = req.locale.as_deref().unwrap_or("en");

    // Resolve the active provider + model through the centralized layer.
    let completer = resolve_completer(&app, &req).map_err(|e| {
        tracing::error!("cover_letter: provider resolution failed: {e}");
        e
    })?;

    let brave_key = app
        .state::<Mutex<crate::credentials::CredentialStore>>()
        .lock()
        .get_decrypted("ai:brave")
        .map(|(_, k)| k);

    let cache = app.state::<CompanyBriefCache>();

    // ── Phase 1: research + resume key-point extraction (concurrent) ──────────
    emit(&app, "cover_letter:research:start", json!({}));

    let resume_clone = req.resume.clone();
    let job_ad_clone = req.job_ad.clone();
    let cache_ref: &CompanyBriefCache = &cache;

    let (research_result, resume_summary) = tokio::join!(
        async {
            if enable_research {
                research::run(&completer, &job_ad_clone, cache_ref, brave_key.as_deref()).await
            } else {
                (String::new(), String::new())
            }
        },
        tokio::task::spawn_blocking(move || extract_resume_summary(&resume_clone))
    );

    let (company, company_brief) = research_result;
    let resume_summary = resume_summary.unwrap_or_else(|_| req.resume.clone());

    emit(
        &app,
        "cover_letter:research:done",
        json!({ "company": company, "briefLen": company_brief.len() }),
    );

    // ── Phase 2: generation (with up to 2 retries on leakage FAIL) ────────────
    let mut retries: u8 = 0;
    let mut text = String::new();
    let mut leakage_verdict: Option<String> = None;

    for attempt in 0..=2u8 {
        emit(
            &app,
            "cover_letter:generation:start",
            json!({ "attempt": attempt }),
        );

        let user_prompt = build_user_prompt(
            &resume_summary,
            &req.job_ad,
            &company_brief,
            mode,
            locale,
        );

        text = completer
            .complete(COVER_LETTER_SYSTEM, &user_prompt)
            .await
            .map_err(|e| {
                tracing::error!("cover_letter: generation failed (attempt {attempt}): {e}");
                e
            })?;

        // Strip any <think>...</think> blocks emitted by local reasoning models
        text = strip_think_blocks(&text);

        emit(
            &app,
            "cover_letter:generation:done",
            json!({ "attempt": attempt, "chars": text.len() }),
        );

        if !enable_leakage {
            break;
        }

        // ── Phase 3: leakage validation ────────────────────────────────────────
        emit(
            &app,
            "cover_letter:validation:start",
            json!({ "attempt": attempt }),
        );

        match leakage::validate(&completer, &req.resume, &req.job_ad, &text).await {
            Ok(result) => {
                let verdict = result.verdict.clone();
                tracing::info!(
                    attempt,
                    verdict = %verdict,
                    issues = result.issues.len(),
                    "cover_letter: validation complete"
                );
                emit(
                    &app,
                    "cover_letter:validation:done",
                    json!({ "attempt": attempt, "verdict": verdict, "issues": result.issues.len() }),
                );
                leakage_verdict = Some(verdict.clone());

                if verdict == "PASS" || attempt == 2 {
                    break;
                }
                retries += 1;
            }
            Err(e) => {
                tracing::warn!("cover_letter: validation error (non-fatal): {e}");
                emit(
                    &app,
                    "cover_letter:validation:done",
                    json!({ "attempt": attempt, "verdict": "SKIPPED", "error": e }),
                );
                leakage_verdict = Some("SKIPPED".to_string());
                break;
            }
        }
    }

    Ok(CoverLetterResponse {
        text,
        company_brief: if company_brief.is_empty() {
            None
        } else {
            Some(company_brief)
        },
        leakage_verdict,
        retries,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Lightweight resume summariser: strips markdown, truncates to 4000 chars.
/// Runs in a blocking thread since it's pure CPU work.
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
