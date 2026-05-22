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
