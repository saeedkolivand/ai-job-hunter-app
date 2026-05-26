use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use crate::credentials::CredentialStore;
use crate::jobs::JobTracker;

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

/// Stream an AI generation from Ollama.
#[tauri::command]
pub async fn ai_generate(app: AppHandle, req: Value) -> Value {
    let provider = req
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("ollama")
        .to_string();

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "ai.generate");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = match provider.as_str() {
            "openai" | "openai-compatible" => {
                let api_key = get_provider_key(&app_clone, &provider).unwrap_or_default();
                let base_url = req
                    .get("baseUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://api.openai.com/v1")
                    .to_string();
                stream_openai_chat(&app_clone, &base_url, &api_key, &job_id_clone, req).await
            }
            "anthropic" => {
                let api_key = get_provider_key(&app_clone, "anthropic").unwrap_or_default();
                stream_anthropic_chat(&app_clone, &api_key, &job_id_clone, req).await
            }
            "gemini" => {
                let api_key = get_provider_key(&app_clone, "gemini").unwrap_or_default();
                stream_gemini_chat(&app_clone, &api_key, &job_id_clone, req).await
            }
            _ => {
                let base = std::env::var("OLLAMA_HOST")
                    .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
                stream_ollama_chat(&app_clone, &base, &job_id_clone, req).await
            }
        };

        if let Err(e) = result {
            let _ = app_clone.emit(
                "ai:stream",
                json!({ "jobId": job_id_clone, "delta": format!("\n\nError: {e}"), "done": true }),
            );
            app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .unwrap()
                .fail(&job_id_clone, e);
        }
    });

    json!({ "jobId": job_id })
}

async fn stream_ollama_chat(
    app: &AppHandle,
    base: &str,
    job_id: &str,
    req: Value,
) -> Result<(), String> {
    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let messages = req.get("messages").cloned().unwrap_or(json!([]));
    let temperature = req.get("temperature").and_then(|v| v.as_f64());
    let max_tokens = req.get("maxTokens").and_then(|v| v.as_u64());

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    let mut options = serde_json::Map::new();
    if let Some(t) = temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(mt) = max_tokens {
        options.insert("num_predict".to_string(), json!(mt));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {body_text}"));
    }

    let mut line_buf = String::new();
    let mut received_any_content = false;

    loop {
        // Check if job was cancelled before reading next chunk
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().unwrap().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                let _ = response.error_for_status_ref();
                return Err("Job cancelled".to_string());
            }
        }

        // After receiving content, use a 30s idle timeout per chunk so we detect
        // when Ollama stops sending without closing the connection.
        let chunk_timeout = if received_any_content {
            std::time::Duration::from_secs(30)
        } else {
            std::time::Duration::from_secs(300)
        };

        let chunk_result = tokio::time::timeout(chunk_timeout, response.chunk()).await;

        match chunk_result {
            Err(_) => {
                // Idle timeout expired — force stream completion
                break;
            }
            Ok(Err(e)) => return Err(format!("Stream error: {e}")),
            Ok(Ok(None)) => break,
            Ok(Ok(Some(bytes))) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    let event: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta = event
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let done = event
                        .get("done")
                        .and_then(|d| d.as_bool())
                        .unwrap_or(false);

                    if done {
                        let _ = app.emit(
                            "ai:stream",
                            json!({ "jobId": job_id, "delta": delta, "done": true }),
                        );
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .complete(job_id, json!({ "done": true }));
                        return Ok(());
                    }

                    // Skip empty keep-alive chunks
                    if delta.is_empty() {
                        continue;
                    }

                    received_any_content = true;

                    let _ = app.emit(
                        "ai:stream",
                        json!({ "jobId": job_id, "delta": delta, "done": false }),
                    );
                }
            }
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .complete(job_id, json!({ "done": true }));

    Ok(())
}

fn get_provider_key(app: &AppHandle, provider: &str) -> Option<String> {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    guard
        .get_decrypted(&format!("ai:{provider}"))
        .map(|(_, password)| password)
}

#[tauri::command]
pub fn ai_set_provider_key(app: AppHandle, provider: String, api_key: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.set(&format!("ai:{provider}"), "apikey", &api_key) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_remove_provider_key(app: AppHandle, provider: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.remove(&format!("ai:{provider}")) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_has_provider_key(app: AppHandle, provider: String) -> Value {
    json!({ "has": get_provider_key(&app, &provider).is_some() })
}

#[tauri::command]
pub async fn ai_test_provider_key(app: AppHandle, provider: String) -> Value {
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!({ "success": false, "error": "No API key found" }),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "error": format!("Failed to create client: {}", e) }),
    };

    let result = match provider.as_str() {
        "openai" | "openai-compatible" => {
            let resp = client
                .get("https://api.openai.com/v1/models")
                .bearer_auth(&api_key)
                .send()
                .await;
            match resp {
                Ok(r) => {
                    if r.status().is_success() {
                        json!({ "success": true })
                    } else {
                        let status = r.status();
                        json!({ "success": false, "error": format!("API returned status: {}", status) })
                    }
                }
                Err(e) => json!({ "success": false, "error": format!("Request failed: {}", e) }),
            }
        }
        "anthropic" => {
            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&serde_json::json!({
                    "model": "claude-3-haiku-20240307",
                    "max_tokens": 1,
                    "messages": [{"role": "user", "content": "test"}]
                }))
                .send()
                .await;
            match resp {
                Ok(r) => {
                    if r.status().is_success() || r.status() == 400 {
                        // 400 is OK - it means key is valid but request was malformed (we sent minimal data)
                        json!({ "success": true })
                    } else {
                        let status = r.status();
                        json!({ "success": false, "error": format!("API returned status: {}", status) })
                    }
                }
                Err(e) => json!({ "success": false, "error": format!("Request failed: {}", e) }),
            }
        }
        "gemini" => {
            let resp = client
                .get(format!("https://generativelanguage.googleapis.com/v1/models?key={}", api_key))
                .send()
                .await;
            match resp {
                Ok(r) => {
                    if r.status().is_success() {
                        json!({ "success": true })
                    } else {
                        let status = r.status();
                        json!({ "success": false, "error": format!("API returned status: {}", status) })
                    }
                }
                Err(e) => json!({ "success": false, "error": format!("Request failed: {}", e) }),
            }
        }
        _ => json!({ "success": false, "error": "Unknown provider" }),
    };

    result
}

#[tauri::command]
pub async fn ai_list_provider_models(app: AppHandle, provider: String) -> Value {
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!([]),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    match provider.as_str() {
        "openai" | "openai-compatible" => {
            let resp = client
                .get("https://api.openai.com/v1/models")
                .bearer_auth(&api_key)
                .send()
                .await;
            if let Ok(r) = resp {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                        let models: Vec<Value> = data
                            .iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                            .filter(|id| {
                                id.starts_with("gpt-")
                                    || id.starts_with("o1")
                                    || id.starts_with("o3")
                                    || id.starts_with("o4")
                            })
                            .map(|id| json!({ "name": id }))
                            .collect();
                        return json!(models);
                    }
                }
            }
            json!([])
        }
        "anthropic" => {
            let resp = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await;
            if let Ok(r) = resp {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                        let models: Vec<Value> = data
                            .iter()
                            .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                            .filter(|id| id.starts_with("claude-"))
                            .map(|id| json!({ "name": id }))
                            .collect();
                        return json!(models);
                    }
                }
            }
            json!([])
        }
        "gemini" => {
            let resp = client
                .get(format!("https://generativelanguage.googleapis.com/v1/models?key={}", api_key))
                .send()
                .await;
            if let Ok(r) = resp {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    if let Some(models) = body.get("models").and_then(|d| d.as_array()) {
                        let filtered: Vec<Value> = models
                            .iter()
                            .filter_map(|m| m.get("name").and_then(|id| id.as_str()))
                            .filter(|id| id.starts_with("models/"))
                            .map(|id| json!({ "name": id.strip_prefix("models/").unwrap_or(id) }))
                            .collect();
                        return json!(filtered);
                    }
                }
            }
            json!([])
        }
        _ => json!([]),
    }
}

async fn stream_openai_chat(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    job_id: &str,
    req: Value,
) -> Result<(), String> {
    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o")
        .to_string();
    let messages = req.get("messages").cloned().unwrap_or(json!([]));
    let temperature = req.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
    let max_tokens = req.get("maxTokens").and_then(|v| v.as_u64());

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
    });
    if let Some(mt) = max_tokens {
        body["max_tokens"] = json!(mt);
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI {status}: {body_text}"));
    }

    let mut line_buf = String::new();

    loop {
        // Check if job was cancelled before reading next chunk
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().unwrap().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if data == "[DONE]" {
                        let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
                        app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
                        return Ok(());
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta = event
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");

                    if !delta.is_empty() {
                        let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": delta, "done": false }));
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

async fn stream_anthropic_chat(
    app: &AppHandle,
    api_key: &str,
    job_id: &str,
    req: Value,
) -> Result<(), String> {
    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let temperature = req.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
    let max_tokens = req.get("maxTokens").and_then(|v| v.as_u64()).unwrap_or(4096);

    let messages_raw = req.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();
    let system_content: String = messages_raw
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let messages: Vec<Value> = messages_raw
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .cloned()
        .collect();

    // Enable extended thinking for balanced effort and above (max_tokens >= 2048).
    // Thinking requires temperature=1; budget is half the output quota.
    let thinking_budget = if max_tokens >= 2048 { max_tokens / 2 } else { 0 };
    let actual_max_tokens = max_tokens + thinking_budget;

    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": actual_max_tokens,
        "stream": true,
        "temperature": if thinking_budget > 0 { 1.0 } else { temperature },
    });
    if thinking_budget > 0 {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": thinking_budget });
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic {status}: {body_text}"));
    }

    let mut line_buf = String::new();
    let mut last_event = String::new();

    loop {
        // Check if job was cancelled before reading next chunk
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().unwrap().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();

                    if let Some(event) = line.strip_prefix("event: ") {
                        last_event = event.trim().to_string();
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if last_event == "message_stop" || data.contains("\"type\":\"message_stop\"") {
                        let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
                        app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
                        return Ok(());
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta_obj = event.get("delta");
                    let delta_type = delta_obj
                        .and_then(|d| d.get("type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    match delta_type {
                        "thinking_delta" => {
                            let thinking = delta_obj
                                .and_then(|d| d.get("thinking"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !thinking.is_empty() {
                                let _ = app.emit("ai:stream", json!({
                                    "jobId": job_id,
                                    "delta": thinking,
                                    "done": false,
                                    "thinking": true,
                                }));
                            }
                        }
                        "text_delta" => {
                            let text = delta_obj
                                .and_then(|d| d.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !text.is_empty() {
                                let _ = app.emit("ai:stream", json!({
                                    "jobId": job_id,
                                    "delta": text,
                                    "done": false,
                                }));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

async fn stream_gemini_chat(
    app: &AppHandle,
    api_key: &str,
    job_id: &str,
    req: Value,
) -> Result<(), String> {
    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini-2.0-flash")
        .to_string();
    let temperature = req.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
    let max_tokens = req.get("maxTokens").and_then(|v| v.as_u64());

    let messages_raw = req.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();

    let system_text: String = messages_raw
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    let contents: Vec<Value> = messages_raw
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .map(|m| {
            let role = if m.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                "model"
            } else {
                "user"
            };
            let text = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            json!({ "role": role, "parts": [{ "text": text }] })
        })
        .collect();

    let mut generation_config = json!({ "temperature": temperature });
    if let Some(mt) = max_tokens {
        generation_config["maxOutputTokens"] = json!(mt);
    }
    let mut body = json!({
        "contents": contents,
        "generationConfig": generation_config,
    });
    if !system_text.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
        model, api_key
    );

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini {status}: {body_text}"));
    }

    let mut buf = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;

    loop {
        // Check if job was cancelled before reading next chunk
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().unwrap().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                let chunk = String::from_utf8_lossy(&bytes).to_string();
                for ch in chunk.chars() {
                    if escape { escape = false; buf.push(ch); continue; }
                    if ch == '\\' && in_string { escape = true; buf.push(ch); continue; }
                    if ch == '"' { in_string = !in_string; }
                    if !in_string {
                        if ch == '{' { depth += 1; }
                        else if ch == '}' { depth -= 1; }
                    }
                    buf.push(ch);

                    if depth == 0 && buf.trim_start().starts_with('{') && !buf.trim().is_empty() {
                        if let Ok(event) = serde_json::from_str::<Value>(buf.trim()) {
                            let delta = event
                                .get("candidates")
                                .and_then(|c| c.get(0))
                                .and_then(|c| c.get("content"))
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.get(0))
                                .and_then(|p| p.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !delta.is_empty() {
                                let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": delta, "done": false }));
                            }
                        }
                        buf.clear();
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

#[tauri::command]
pub async fn ai_list_models() -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    let resp = match client.get(format!("{base}/api/tags")).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return json!([]),
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return json!([]),
    };

    let models: Vec<serde_json::Value> = body
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|name| json!({ "name": name }))
                .collect()
        })
        .unwrap_or_default();

    json!(models)
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "ai.pull_model");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = pull_ollama_model(&app_clone, &base, &job_id_clone, &model).await;
        match result {
            Ok(()) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .unwrap()
                    .complete(&job_id_clone, json!({ "model": model, "done": true }));
            }
            Err(e) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .unwrap()
                    .fail(&job_id_clone, e);
            }
        }
    });

    json!({ "jobId": job_id })
}

async fn pull_ollama_model(
    app: &AppHandle,
    base: &str,
    job_id: &str,
    model: &str,
) -> Result<(), String> {
    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base}/api/pull"))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {body}"));
    }

    let mut line_buf = String::new();

    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
        line_buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(nl) = line_buf.find('\n') {
            let line = line_buf[..nl].trim().to_string();
            line_buf = line_buf[nl + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let event: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let status = event.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let completed = event.get("completed").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let total = event.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let digest = event.get("digest").and_then(|v| v.as_str()).unwrap_or("");
            let p = if total > 0.0 { completed / total } else { 0.0 };

            let _ = app.emit("jobs:event", json!({ "type": "job.stream", "jobId": job_id, "data": { "status": status, "p": p, "completed": completed, "total": total, "digest": digest } }));

            if status == "success" {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!({ "success": true })
}

#[tauri::command]
pub async fn ai_embed(req: Value) -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let model = req
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("nomic-embed-text");

    let body = json!({ "model": model, "prompt": text });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!(null),
    };

    let resp = match client.post(format!("{base}/api/embeddings")).json(&body).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return json!(null),
    };

    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return json!(null),
    };

    let vector = data.get("embedding").cloned().unwrap_or(json!([]));
    let dim = vector.as_array().map(|a| a.len()).unwrap_or(0);
    json!({ "vector": vector, "dim": dim })
}
