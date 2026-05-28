use crate::cover_letter::Completer;

const SYSTEM: &str = "\
You are a leakage detector. \
Find sentences in a generated cover letter that make factual claims about the candidate \
NOT supported by the candidate's resume. \
Return ONLY valid JSON — no prose, no markdown, no code fences.";

#[derive(Debug, serde::Serialize)]
pub struct LeakageIssue {
    pub sentence: String,
    pub classification: String, // "LEAKED" | "FABRICATED"
    pub reason: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LeakageResult {
    pub verdict: String, // "PASS" | "FAIL"
    pub issues: Vec<LeakageIssue>,
}

/// Run the leakage validation pass.
///
/// Returns `Ok(LeakageResult)` on success. Returns `Err` only on hard
/// LLM/network failures — a FAIL verdict is returned as `Ok(...)`, not `Err`.
pub async fn validate(
    llm: &Completer,
    resume: &str,
    job_ad: &str,
    generated: &str,
) -> Result<LeakageResult, String> {
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
        resume = &resume[..resume.len().min(4000)],
        job_ad = &job_ad[..job_ad.len().min(2000)],
        generated = &generated[..generated.len().min(4000)],
    );

    let raw = llm.complete(SYSTEM, &user).await?;
    parse_result(&raw).ok_or_else(|| format!("leakage: could not parse response: {raw}"))
}

fn parse_result(raw: &str) -> Option<LeakageResult> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let json_str = &raw[start..=end];
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;

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
                    Some(LeakageIssue {
                        sentence: item.get("sentence")?.as_str()?.to_string(),
                        classification: item.get("classification")?.as_str()?.to_string(),
                        reason: item
                            .get("reason")
                            .and_then(|r| r.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(LeakageResult {
        verdict: verdict.to_string(),
        issues,
    })
}
