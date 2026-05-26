use serde_json::json;

use super::{build_client, LlmProvider};

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: Option<String>, base_url: Option<String>) -> Result<Self, String> {
        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: build_client()?,
        })
    }

    async fn call(&self, model: &str, system: &str, user: &str) -> Result<String, String> {
        let body = json!({
            "model": model,
            "max_tokens": 2048,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user",   "content": user   },
            ],
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("openai request: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("openai {status}: {text}"));
        }

        let data: serde_json::Value =
            resp.json().await.map_err(|e| format!("openai parse: {e}"))?;
        data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .ok_or_else(|| "openai: unexpected response shape".to_string())
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        self.call(&self.model.clone(), system, user).await
    }
}
