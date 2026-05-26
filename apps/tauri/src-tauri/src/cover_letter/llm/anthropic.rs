use serde_json::json;

use super::{build_client, LlmProvider};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>) -> Result<Self, String> {
        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
            client: build_client()?,
        })
    }

    async fn call(&self, model: &str, system: &str, user: &str) -> Result<String, String> {
        let body = json!({
            "model": model,
            "max_tokens": 2048,
            "system": system,
            "messages": [{ "role": "user", "content": user }],
        });

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("anthropic request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("anthropic {status}: {text}"));
        }

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("anthropic parse: {e}"))?;
        data.get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .ok_or_else(|| "anthropic: unexpected response shape".to_string())
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        self.call(&self.model.clone(), system, user).await
    }
}
