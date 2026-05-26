use serde_json::json;

use super::LlmProvider;

pub struct OllamaProvider {
    base: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(model: String) -> Result<Self, String> {
        let base = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
        Ok(Self {
            base,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| format!("ollama client build: {e}"))?,
        })
    }
}

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        let body = json!({
            "model": self.model,
            "stream": false,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user",   "content": user   },
            ],
        });

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("ollama request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("ollama {status}: {text}"));
        }

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("ollama parse: {e}"))?;
        data.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(String::from)
            .ok_or_else(|| "ollama: unexpected response shape".to_string())
    }
}
