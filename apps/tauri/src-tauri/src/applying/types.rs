#![allow(dead_code)]

use tokio_util::sync::CancellationToken;

pub struct ApplyContext {
    pub signal: CancellationToken,
    pub cover_letter: Option<String>,
    pub resume_path: Option<String>,
    pub auto_submit: bool,
    pub on_progress: Option<Box<dyn Fn(f32, String) + Send>>,
    pub on_step: Option<Box<dyn Fn(ApplyStep) + Send>>,
}

#[derive(Debug, Clone)]
pub struct ApplyStep {
    pub stage: String,
    pub ok: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub ok: bool,
    pub stage: String,
    pub submitted: bool,
    pub url: String,
    pub note: Option<String>,
}

#[async_trait::async_trait]
pub trait Applier: Send + Sync {
    fn board_id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    
    async fn apply(&self, posting_url: String, ctx: ApplyContext) -> Result<ApplyResult, anyhow::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_step_creation() {
        let step = ApplyStep {
            stage: "filling_form".to_string(),
            ok: true,
            note: Some("Successfully filled form".to_string()),
        };
        assert_eq!(step.stage, "filling_form");
        assert!(step.ok);
        assert_eq!(step.note, Some("Successfully filled form".to_string()));
    }

    #[test]
    fn test_apply_step_defaults() {
        let step = ApplyStep {
            stage: "navigation".to_string(),
            ok: false,
            note: None,
        };
        assert_eq!(step.stage, "navigation");
        assert!(!step.ok);
        assert!(step.note.is_none());
    }

    #[test]
    fn test_apply_result_creation() {
        let result = ApplyResult {
            ok: true,
            stage: "submitted".to_string(),
            submitted: true,
            url: "https://example.com/job/123".to_string(),
            note: Some("Application submitted successfully".to_string()),
        };
        assert!(result.ok);
        assert!(result.submitted);
        assert_eq!(result.url, "https://example.com/job/123");
    }

    #[test]
    fn test_apply_result_defaults() {
        let result = ApplyResult {
            ok: false,
            stage: "failed".to_string(),
            submitted: false,
            url: "https://example.com/job/123".to_string(),
            note: None,
        };
        assert!(!result.ok);
        assert!(!result.submitted);
        assert!(result.note.is_none());
    }

    #[test]
    fn test_apply_result_clone() {
        let result = ApplyResult {
            ok: true,
            stage: "test".to_string(),
            submitted: true,
            url: "https://example.com".to_string(),
            note: None,
        };
        let cloned = result.clone();
        assert_eq!(result.stage, cloned.stage);
        assert_eq!(result.ok, cloned.ok);
    }

    #[test]
    fn test_apply_step_clone() {
        let step = ApplyStep {
            stage: "test".to_string(),
            ok: true,
            note: Some("test note".to_string()),
        };
        let cloned = step.clone();
        assert_eq!(step.stage, cloned.stage);
        assert_eq!(step.ok, cloned.ok);
    }
}
