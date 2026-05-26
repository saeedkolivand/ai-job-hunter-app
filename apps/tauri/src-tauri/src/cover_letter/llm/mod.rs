pub mod anthropic;
pub mod ollama;
pub mod openai;

/// All LLM providers implement this single trait.
/// `complete` is a non-streaming call — the full response is needed before
/// the leakage validator can run, so streaming offers no pipeline benefit here.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> Result<String, String>;
}

/// Shared rustls-backed HTTP client for all cover-letter API calls.
/// Built once per provider instance and reused across calls.
pub fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client build: {e}"))
}
