pub mod cache;
pub mod leakage;
pub mod llm;
pub mod research;

use std::sync::Mutex;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use cache::CompanyBriefCache;
use llm::{
    anthropic::AnthropicProvider, ollama::OllamaProvider, openai::OpenAiProvider, LlmProvider,
};

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

// ── LLM provider factory ──────────────────────────────────────────────────────

/// Resolve the provider name and build the concrete `LlmProvider`.
/// API keys are read directly from the OS keychain via `CredentialStore` —
/// they never travel through the frontend.
fn resolve_provider(
    app: &AppHandle,
    provider_hint: Option<&str>,
    model: Option<&str>,
    base_url: Option<&str>,
) -> Result<(Box<dyn LlmProvider>, Box<dyn LlmProvider>), String> {
    let get_key = |name: &str| -> Option<String> {
        app.state::<Mutex<crate::credentials::CredentialStore>>()
            .lock()
            .unwrap()
            .get_decrypted(&format!("ai:{name}"))
            .map(|(_, k)| k)
    };

    // Determine provider: use hint, or auto-select from what has a key
    let provider = provider_hint
        .map(String::from)
        .or_else(|| {
            for p in &["anthropic", "openai", "gemini", "openai-compatible"] {
                if get_key(p).is_some() {
                    return Some(p.to_string());
                }
            }
            None
        })
        .unwrap_or_else(|| "ollama".to_string());

    match provider.as_str() {
        "anthropic" => {
            let key = get_key("anthropic")
                .ok_or_else(|| "anthropic API key not set — add it in Settings".to_string())?;
            // gen uses the caller's model choice (or Sonnet); fast uses Haiku for ancillary calls
            let gen = AnthropicProvider::new(key.clone(), model.map(String::from))?;
            let fast = AnthropicProvider::new(
                key,
                Some("claude-haiku-4-5-20251001".to_string()),
            )?;
            Ok((Box::new(gen), Box::new(fast)))
        }
        "openai" | "openai-compatible" => {
            let key = get_key(&provider)
                .ok_or_else(|| format!("{provider} API key not set — add it in Settings"))?;
            let gen = OpenAiProvider::new(
                key.clone(),
                model.map(String::from),
                base_url.map(String::from),
            )?;
            // gpt-4o-mini for research briefs and validation
            let fast = OpenAiProvider::new(
                key,
                Some("gpt-4o-mini".to_string()),
                base_url.map(String::from),
            )?;
            Ok((Box::new(gen), Box::new(fast)))
        }
        _ => {
            // Ollama — local, no key needed; same model for all passes
            let m = model.unwrap_or("llama3.2").to_string();
            let gen = OllamaProvider::new(m.clone())?;
            let fast = OllamaProvider::new(m)?;
            Ok((Box::new(gen), Box::new(fast)))
        }
    }
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

    // Build providers (gen-quality + fast for ancillary calls)
    let (gen_llm, fast_llm) = resolve_provider(
        &app,
        req.provider.as_deref(),
        req.model.as_deref(),
        req.base_url.as_deref(),
    )
    .map_err(|e| {
        tracing::error!("cover_letter: provider resolution failed: {e}");
        e
    })?;

    let brave_key = app
        .state::<Mutex<crate::credentials::CredentialStore>>()
        .lock()
        .unwrap()
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
                research::run(
                    fast_llm.as_ref(),
                    &job_ad_clone,
                    cache_ref,
                    brave_key.as_deref(),
                )
                .await
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

        text = gen_llm
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

        match leakage::validate(fast_llm.as_ref(), &req.resume, &req.job_ad, &text).await {
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
