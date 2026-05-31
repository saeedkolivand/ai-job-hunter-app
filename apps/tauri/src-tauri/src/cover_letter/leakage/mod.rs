use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::Emitter;

use crate::error::AppResult;
use crate::pipeline::validation::{ValidationIssue, ValidationReport, Validator};
use crate::pipeline::Completer;

const SYSTEM: &str = "\
You are a leakage detector. \
Find sentences in a generated cover letter that make factual claims about the candidate \
NOT supported by the candidate's resume. \
Return ONLY valid JSON — no prose, no markdown, no code fences.";

/// Detects claims in a generated draft that are not supported by the candidate's
/// resume (leaked from the job ad or fabricated). A reusable
/// [`crate::pipeline::validation::Validator`] — the regenerate loop lives in
/// [`crate::pipeline::retry`].
pub struct LeakageValidator {
    resume: String,
    job_ad: String,
}

impl LeakageValidator {
    pub fn new(resume: String, job_ad: String) -> Self {
        Self { resume, job_ad }
    }
}

#[async_trait]
impl Validator for LeakageValidator {
    fn name(&self) -> &'static str {
        "leakage"
    }

    async fn validate(&self, completer: &Completer, draft: &str) -> AppResult<ValidationReport> {
        let _ = completer
            .app()
            .emit("cover_letter:validation:start", json!({}));

        let user = format!(
            "<original_resume>\n{resume}\n</original_resume>\n\n\
             <original_job_ad>\n{job_ad}\n</original_job_ad>\n\n\
             <generated_document>\n{generated}\n</generated_document>\n\n\
             For each sentence in <generated_document> that makes a factual claim about the candidate, \
             classify it as:\n\
               SUPPORTED   — backed by <original_resume>\n\
               LEAKED      — appears to come from <original_job_ad>\n\
               FABRICATED  — in neither source\n\
               STYLISTIC   — not a factual claim (greetings, transitions, etc.)\n\n\
             Output JSON only:\n\
             {{\n\
               \"verdict\": \"PASS\" | \"FAIL\",\n\
               \"issues\": [\n\
                 {{ \"sentence\": \"...\", \"classification\": \"LEAKED\" | \"FABRICATED\", \"reason\": \"...\" }}\n\
               ]\n\
             }}\n\n\
             PASS only if there are zero LEAKED or FABRICATED items. Return ONLY the JSON object.",
            resume = &self.resume[..self.resume.len().min(4000)],
            job_ad = &self.job_ad[..self.job_ad.len().min(2000)],
            generated = &draft[..draft.len().min(4000)],
        );

        let raw = completer.complete(SYSTEM, &user, Some(0.2)).await?;
        let report = parse_report(&raw)
            .ok_or_else(|| format!("leakage: could not parse response: {raw}"))?;

        let _ = completer.app().emit(
            "cover_letter:validation:done",
            json!({ "verdict": report.verdict, "issues": report.issues.len() }),
        );
        Ok(report)
    }
}

fn parse_report(raw: &str) -> Option<ValidationReport> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let v: Value = serde_json::from_str(&raw[start..=end]).ok()?;

    let verdict = match v.get("verdict")?.as_str()? {
        "PASS" => "PASS",
        "FAIL" => "FAIL",
        _ => return None,
    };

    let issues = v
        .get("issues")
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let sentence = item.get("sentence")?.as_str()?.to_string();
                    let kind = item.get("classification")?.as_str()?.to_string();
                    let reason = item
                        .get("reason")
                        .and_then(|r| r.as_str())
                        .unwrap_or("")
                        .to_string();
                    let detail = if reason.is_empty() {
                        sentence
                    } else {
                        format!("{sentence} — {reason}")
                    };
                    Some(ValidationIssue { kind, detail })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ValidationReport {
        passed: verdict == "PASS",
        verdict: verdict.to_string(),
        issues,
    })
}
