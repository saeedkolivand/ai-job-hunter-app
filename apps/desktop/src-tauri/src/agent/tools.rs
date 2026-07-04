//! Tool registry: fixed, trusted adapters over existing read-only commands.
//!
//! Whitelists are per-flow slices — there is deliberately NO global "all commands"
//! tool (least privilege, OWASP LLM06 Excessive Agency). A tool's `schema` and
//! `description` are fixed, trusted strings — never built from scraped or
//! model-supplied text. The handlers are thin adapters that delegate to the
//! existing Tauri commands; no business logic is duplicated here.

use std::future::Future;
use std::pin::Pin;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai_provider::ToolSpec;
use crate::error::AppResult;

/// Whether a tool only reads (safe to auto-run) or writes/spends (DENIED in Phase
/// 1 until user-confirmation lands in Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Write,
}

/// A tool's async handler: takes the app handle + the model-supplied (untrusted)
/// arguments and returns a JSON result. The returned future is `'static` (each
/// handler clones the `AppHandle` it needs) so it fits a plain `fn` pointer.
pub type ToolHandler =
    fn(&AppHandle, Value) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>>;

/// One registered tool: a fixed name + description + argument schema, its safety
/// [`ToolKind`], and the handler that runs it.
pub struct AgentTool {
    pub name: &'static str,
    pub description: String,
    pub schema: Value,
    pub kind: ToolKind,
    pub handler: ToolHandler,
}

/// Turn a per-flow whitelist into the provider-facing [`ToolSpec`] list handed to
/// the model.
#[allow(dead_code)] // ponytail: wired in Phase 2 (agent_run)
pub fn to_specs(tools: &[AgentTool]) -> Vec<ToolSpec> {
    tools
        .iter()
        .map(|t| ToolSpec {
            name: t.name.to_string(),
            description: t.description.clone(),
            schema: t.schema.clone(),
        })
        .collect()
}

// ── Read tools (thin adapters over existing commands — no business logic here) ──

fn research_company_handler(
    app: &AppHandle,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    Box::pin(async move {
        // SECURITY (lethal-trifecta exfil leg): `args` is model-supplied and thus
        // untrusted — a prompt-injected job posting could otherwise stuff a
        // `baseUrl`/`provider`/`model` field into the call and redirect the
        // credentialed provider request to an attacker host (SSRF / API-key
        // exfil). Read ONLY the schema-declared fields (`jobAd`, `company`);
        // ponytail: provider/model/baseUrl come from trusted caller context in
        // Phase 2, NEVER from tool args. Passing `None` here matches how
        // `ai_research_company` already degrades gracefully (empty brief) when a
        // caller doesn't supply an override.
        let job_ad = args
            .get("jobAd")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let company = args
            .get("company")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Ok(crate::commands::ai::ai_research_company(app, job_ad, company, None, None, None).await)
    })
}

fn match_resume_handler(
    app: &AppHandle,
    args: Value,
) -> Pin<Box<dyn Future<Output = AppResult<Value>> + Send>> {
    let app = app.clone();
    Box::pin(async move {
        // MatchResumeRequest is camelCase — `resumeId`/`jobId`/`semanticScoringEnabled`.
        let req = serde_json::from_value(args)?;
        Ok(crate::commands::match_resume::match_resume(app, req).await)
    })
}

/// The default read-only whitelist: company research + résumé/job matching, both
/// thin adapters over the existing Tauri commands (reused, not re-implemented).
/// A per-flow caller picks the slice of tools it wants to expose.
#[allow(dead_code)] // ponytail: wired in Phase 2 (agent_run)
pub fn read_tools() -> Vec<AgentTool> {
    vec![
        AgentTool {
            name: "research_company",
            description:
                "Research a company from a job posting and return a short factual brief. Read-only."
                    .to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "jobAd": {
                        "type": "string",
                        "description": "The job posting text to extract the company from."
                    },
                    "company": {
                        "type": "string",
                        "description": "Optional explicit company name."
                    }
                },
                "required": ["jobAd"]
            }),
            kind: ToolKind::Read,
            handler: research_company_handler,
        },
        AgentTool {
            name: "match_resume",
            description:
                "Score how well a résumé matches a job posting (ATS + semantic). Read-only."
                    .to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "resumeId": { "type": "string" },
                    "jobId": { "type": "string" },
                    "semanticScoringEnabled": { "type": "boolean" }
                },
                "required": ["resumeId", "jobId"]
            }),
            kind: ToolKind::Read,
            handler: match_resume_handler,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tools_are_all_read_kind_and_convert_to_specs() {
        let tools = read_tools();
        assert!(!tools.is_empty());
        assert!(
            tools.iter().all(|t| t.kind == ToolKind::Read),
            "the default whitelist must be read-only"
        );
        let specs = to_specs(&tools);
        assert_eq!(specs.len(), tools.len());
        // Names + schemas carry through so the provider sees the same whitelist.
        assert_eq!(specs[0].name, tools[0].name);
        assert!(specs.iter().any(|s| s.name == "research_company"));
        assert!(specs.iter().any(|s| s.name == "match_resume"));
    }
}
