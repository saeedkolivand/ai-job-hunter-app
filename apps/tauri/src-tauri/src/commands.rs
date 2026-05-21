/// Tauri command implementations for the AJH shell.
///
/// Real commands: system_health/version/platform/locale/openExternal,
///                scrape_board, scrape_url (proxy to scraper sidecar),
///                credentials_* (OS keychain via keyring crate),
///                dialog_open_files.
///
/// Stub commands: everything else — they return null / empty list so the UI
/// renders with empty states. Parity is built incrementally.
use std::sync::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use crate::autopilot::{AutopilotStatus, AutopilotStore};
use crate::credentials::CredentialStore;
use crate::jobs::JobTracker;
use crate::postings::{InteractionRecord, InteractionStore, PostingsCache};
use crate::sidecar::ScraperSidecarState;

// ── Sidecar HTTP helpers ──────────────────────────────────────────────────────

fn sidecar_port(app: &AppHandle) -> Option<u16> {
    app.state::<Mutex<ScraperSidecarState>>()
        .lock()
        .ok()
        .and_then(|g| g.port)
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

/// POST a ScraperCommand to the sidecar, stream SSE events back, and
/// update JobTracker + PostingsCache while events arrive.
async fn post_sidecar_command(
    app: &AppHandle,
    port: u16,
    cmd: &Value,
    job_id: &str,
) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}/command");

    let response = reqwest::Client::new()
        .post(&url)
        .json(cmd)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = response.text().await.map_err(|e| e.to_string())?;
    let mut last_done: Value = json!(null);

    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<Value>(data) {
                let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                match kind {
                    "done" => {
                        last_done = event.get("result").cloned().unwrap_or(json!(null));
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .complete(job_id, last_done.clone());
                        let _ = app.emit("jobs:event", json!({"type":"completed","jobId":job_id}));
                    }
                    "progress" => {
                        let p = event.get("p").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .update_progress(job_id, p);
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "item" => {
                        if let Some(item) = event.get("item").cloned() {
                            app.state::<Mutex<PostingsCache>>()
                                .lock()
                                .unwrap()
                                .add(item);
                        }
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "error" => {
                        let msg = event
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("sidecar error")
                            .to_string();
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .fail(job_id, msg.clone());
                        return Err(msg);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(last_done)
}

// ── System ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn system_health(app: AppHandle) -> Value {
    let scraper_ready = sidecar_port(&app).is_some();

    // Check Ollama availability and get the running model if any.
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    let ollama_resp = client.get(format!("{base}/api/tags")).send().await;
    let (ai_ready, ai_model) = match ollama_resp {
        Ok(r) if r.status().is_success() => {
            let body: serde_json::Value = r.json().await.unwrap_or_default();
            let model = body
                .get("models")
                .and_then(|m| m.as_array())
                .and_then(|arr| arr.first())
                .and_then(|m| m.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            (true, model)
        }
        _ => (false, None),
    };

    json!({
        "status": "ok",
        "shell": "tauri",
        "scraper": { "mode": "http-sidecar", "ready": scraper_ready },
        "ai": { "ready": ai_ready, "model": ai_model },
        "data": { "ready": true, "sqlite": true, "vector": true },
        "workers": { "active": 0, "idle": 1, "max": 1 }
    })
}

#[tauri::command]
pub fn system_get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn system_get_locale(app: AppHandle) -> Value {
    let locale = read_locale_file(&app);
    json!(locale)
}

#[tauri::command]
pub fn system_set_locale(app: AppHandle, locale: String) -> Value {
    write_locale_file(&app, &locale);
    json!(null)
}

fn locale_file_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("locale.json")
}

fn read_locale_file(app: &AppHandle) -> String {
    std::fs::read_to_string(locale_file_path(app))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("locale").and_then(|l| l.as_str()).map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn write_locale_file(app: &AppHandle, locale: &str) {
    let path = locale_file_path(app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let content = serde_json::json!({ "locale": locale });
    std::fs::write(path, serde_json::to_string(&content).unwrap_or_default()).ok();
}

#[tauri::command]
pub fn system_get_platform() -> Value {
    json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "shell": "tauri"
    })
}

#[tauri::command]
pub async fn system_open_external(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn system_set_performance_mode(app: AppHandle, mode: String) -> Value {
    // Forward to the sidecar so scraping concurrency is adjusted live.
    if let Some(port) = sidecar_port(&app) {
        let job_id = uuid_v4();
        let cmd = json!({ "kind": "set.performance_mode", "mode": mode });
        post_sidecar_command(&app, port, &cmd, &job_id).await.ok();
    }
    json!(null)
}

#[tauri::command]
pub fn system_get_metrics() -> Value {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let uptime = System::uptime();
    let cpu_percent = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
        / sys.cpus().len().max(1) as f32;

    json!({
        "shell": "tauri",
        "uptime": uptime,
        "memoryMb": used_mem / 1024 / 1024,
        "totalMemoryMb": total_mem / 1024 / 1024,
        "cpuPercent": (cpu_percent * 10.0).round() / 10.0
    })
}

// ── Jobs ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn jobs_list(app: AppHandle) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock().unwrap();
    json!(guard.list())
}

#[tauri::command]
pub fn jobs_get(app: AppHandle, job_id: String) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock().unwrap();
    json!(guard.get(&job_id))
}

#[tauri::command]
pub async fn jobs_cancel(app: AppHandle, job_id: String) -> Value {
    // Tell the sidecar to abort then update local tracker.
    if let Some(port) = sidecar_port(&app) {
        let cmd = json!({ "kind": "cancel", "jobId": job_id });
        reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/command"))
            .json(&cmd)
            .send()
            .await
            .ok();
    }
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .cancel(&job_id);
    json!({ "success": true })
}

#[tauri::command]
pub fn jobs_retry(_app: AppHandle, _job_id: String) -> Value {
    // Retry is a no-op in the sidecar model — the caller re-enqueues.
    json!({ "success": false, "reason": "retry not supported in sidecar mode" })
}

// ── AI ───────────────────────────────────────────────────────────────────────

/// Stream an AI generation from Ollama.
///
/// Calls POST <OLLAMA_HOST>/api/chat with streaming enabled.
/// Each Ollama chunk (`{"message":{"content":"..."},"done":false}`) is
/// immediately forwarded to the renderer as a Tauri `ai:stream` event with
/// shape `{ jobId, delta, done }` — the same shape Electron sends over IPC.
///
/// Returns `{ jobId }` immediately so the renderer can subscribe to events
/// before the first chunk arrives.
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
                // Default: Ollama
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

/// Inner streaming logic — runs in a spawned task.
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

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if let Some(t) = temperature {
        body["options"] = json!({ "temperature": t });
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300)) // 5 min — long generations
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

    // response.chunk() reads the stream incrementally without needing futures-util.
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|e| e.to_string())?
    {
        line_buf.push_str(&String::from_utf8_lossy(&bytes));

        // Process all complete newline-terminated lines in the buffer.
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

            let _ = app.emit(
                "ai:stream",
                json!({ "jobId": job_id, "delta": delta, "done": done }),
            );

            if done {
                app.state::<Mutex<JobTracker>>()
                    .lock()
                    .unwrap()
                    .complete(job_id, json!({ "done": true }));
                return Ok(());
            }
        }
    }

    // Stream ended without an explicit done=true — emit final done event.
    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .complete(job_id, json!({ "done": true }));

    Ok(())
}

// ── Cloud AI provider helpers ─────────────────────────────────────────────────

/// Read a provider API key from the OS keychain.
/// Keys are stored with board_id = "ai:{provider}" e.g. "ai:openai".
fn get_provider_key(app: &AppHandle, provider: &str) -> Option<String> {
    use crate::credentials::CredentialStore;
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    guard
        .get_decrypted(&format!("ai:{provider}"))
        .map(|(_, password)| password)
}

/// Store a cloud AI provider API key in the OS keychain.
#[tauri::command]
pub fn ai_set_provider_key(app: AppHandle, provider: String, api_key: String) -> Value {
    use crate::credentials::CredentialStore;
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.set(&format!("ai:{provider}"), "apikey", &api_key) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// Remove a cloud AI provider API key from the OS keychain.
#[tauri::command]
pub fn ai_remove_provider_key(app: AppHandle, provider: String) -> Value {
    use crate::credentials::CredentialStore;
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.remove(&format!("ai:{provider}")) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// Check whether an API key is stored for a provider (does not return the key).
#[tauri::command]
pub fn ai_has_provider_key(app: AppHandle, provider: String) -> Value {
    json!({ "has": get_provider_key(&app, &provider).is_some() })
}

/// Fetch available models from a cloud provider using the stored API key.
/// Returns [] gracefully if the key is missing or the request fails.
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
            // Fallback hardcoded list
            json!([
                { "name": "gpt-4o" }, { "name": "gpt-4o-mini" },
                { "name": "gpt-4-turbo" }, { "name": "gpt-3.5-turbo" },
                { "name": "o1" }, { "name": "o1-mini" }
            ])
        }
        "anthropic" => json!([
            { "name": "claude-opus-4-7" },
            { "name": "claude-sonnet-4-6" },
            { "name": "claude-haiku-4-5-20251001" }
        ]),
        "gemini" => json!([
            { "name": "gemini-2.0-flash" },
            { "name": "gemini-1.5-pro" },
            { "name": "gemini-1.5-flash" },
            { "name": "gemini-1.0-pro" }
        ]),
        _ => json!([]),
    }
}

// ── Cloud provider streaming ───────────────────────────────────────────────────

/// Stream from OpenAI-compatible API (OpenAI, Groq, Together, LM Studio, etc.)
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

    let body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
    });

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

    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
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

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

/// Stream from Anthropic Claude API.
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

    // Anthropic separates system messages from user/assistant messages.
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

    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": 4096,
        "stream": true,
        "temperature": temperature,
    });
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

    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
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

            let delta = event
                .get("delta")
                .and_then(|d| d.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            if !delta.is_empty() {
                let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": delta, "done": false }));
            }
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

/// Stream from Google Gemini API.
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

    let messages_raw = req.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();

    // Gemini uses "user"/"model" roles; extract system prompt into systemInstruction.
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

    let mut body = json!({
        "contents": contents,
        "generationConfig": { "temperature": temperature },
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

    // Gemini streams a JSON array — accumulate chunks and parse each complete object.
    let mut buf = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;

    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
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

            // Each top-level object is a complete candidate chunk.
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

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>().lock().unwrap().complete(job_id, json!({ "done": true }));
    Ok(())
}

/// List models available in the local Ollama instance.
///
/// Calls GET <OLLAMA_HOST>/api/tags (default http://127.0.0.1:11434).
/// Returns [] gracefully if Ollama is not running — the renderer shows an
/// empty model picker rather than an error.
/// Shape matches the Electron router: Array<{ name: string }>.
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

/// Pull (download) a model from Ollama with streaming progress events.
///
/// Returns { jobId } immediately; progress is broadcast via `jobs:event`
/// so the monitoring page updates in real time.
/// Ollama stream: POST /api/pull → newline-delimited JSON
///   {"status":"downloading","completed":N,"total":M} per chunk
///   {"status":"success"} on completion
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
        .timeout(std::time::Duration::from_secs(3600)) // 1 h — large models
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
            let p = if total > 0.0 { completed / total } else { 0.0 };

            let _ = app.emit("jobs:event", json!({ "type": "job.stream", "jobId": job_id, "data": { "status": status, "p": p } }));

            if status == "success" {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    // Ollama unloads models on its own idle timer; no action needed.
    json!({ "success": true })
}

/// Generate an embedding vector for a text string via Ollama.
///
/// Calls POST <OLLAMA_HOST>/api/embeddings.
/// Returns `{ vector: number[], dim: number }` matching the Electron router
/// shape so downstream callers (match/resume scoring, semantic search) work
/// unchanged. Returns null gracefully if Ollama is unavailable.
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

// ── Documents ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn documents_list(app: AppHandle) -> Value {
    let Ok(store) = app.try_state::<crate::documents::DocumentStore>().ok_or(()) else {
        return json!([]);
    };
    serde_json::to_value(store.list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn documents_import(app: AppHandle, req: Value) -> Value {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let locale = req.get("locale").and_then(|v| v.as_str()).map(String::from);

    let bytes_b64 = match req.get("bytes") {
        Some(serde_json::Value::Array(arr)) => {
            let bytes: Vec<u8> = arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect();
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        }
        _ => return json!({ "error": "bytes field missing or invalid" }),
    };

    // Decode base64 back to bytes for native extraction.
    let bytes = {
        use base64::Engine;
        match base64::engine::general_purpose::STANDARD.decode(&bytes_b64) {
            Ok(b) => b,
            Err(e) => return json!({ "error": format!("base64 decode: {e}") }),
        }
    };

    let text = match crate::documents::extract_text(&name, &bytes) {
        Ok(t) => t,
        Err(e) => return json!({ "error": e }),
    };

    let id = crate::documents::make_doc_id();
    let title = crate::documents::strip_extension(&name);
    let rec = crate::documents::DocumentRecord {
        id: id.clone(),
        title,
        name,
        locale,
        text: text.clone(),
        pages: None,
        created_at: crate::documents::now_ms(),
        indexed: false,
    };

    let Ok(store) = app.try_state::<crate::documents::DocumentStore>().ok_or(()) else {
        return json!({ "error": "document store unavailable" });
    };
    if let Err(e) = store.insert(&rec) {
        return json!({ "error": e });
    }

    if let Some(vector) = crate::documents::embed(&text).await {
        let _ = store.upsert_vector(&id, &vector);
        let _ = store.set_indexed(&id);
    }

    let doc = store.list().into_iter().find(|d| d.id == id).unwrap_or(rec);
    json!({ "jobId": id, "document": serde_json::to_value(doc).unwrap_or(json!({})) })
}

#[tauri::command]
pub async fn documents_remove(app: AppHandle, id: String) -> Value {
    if let Some(store) = app.try_state::<crate::documents::DocumentStore>() {
        store.remove(&id).ok();
    }
    json!(null)
}

// ── Search ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn search_hybrid(app: AppHandle, req: Value) -> Value {
    let query = req.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let top_k = req.get("topK").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let query_vec = match crate::documents::embed(query).await {
        Some(v) => v,
        None => return json!({ "items": [], "total": 0 }),
    };

    let Ok(store) = app.try_state::<crate::documents::DocumentStore>().ok_or(()) else {
        return json!({ "items": [], "total": 0 });
    };

    let mut scored: Vec<(String, f64)> = store
        .all_vectors()
        .into_iter()
        .map(|(doc_id, vec)| {
            let score = crate::documents::cosine_similarity(&query_vec, &vec);
            (doc_id, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);

    let docs = store.list();
    let items: Vec<Value> = scored
        .into_iter()
        .filter_map(|(doc_id, score)| {
            docs.iter().find(|d| d.id == doc_id).map(|d| {
                let mut v = serde_json::to_value(d).unwrap_or(json!({}));
                v["_score"] = json!(score);
                v
            })
        })
        .collect();

    let total = items.len();
    json!({ "items": items, "total": total })
}

// ── Scrape ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn scrape_board(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready — start the scraper-runtime binary" });
    };
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.board");
    let cmd = json!({ "kind": "scrape.board", "jobId": job_id, "payload": req });
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(_) => json!({ "jobId": job_id }),
        Err(e) => json!({ "error": e, "jobId": job_id }),
    }
}

#[tauri::command]
pub async fn scrape_url(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready — start the scraper-runtime binary" });
    };
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.url");
    let url_str = req.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let cmd = json!({ "kind": "scrape.url", "jobId": job_id, "payload": { "url": url_str } });
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub fn scrape_persist_job(app: AppHandle, req: Value) -> Value {
    // Record a user interaction with a job (viewed, applied, bookmarked, etc.)
    if let (Some(job_obj), Some(interaction_type)) = (
        req.get("job"),
        req.get("interactionType").and_then(|v| v.as_str()),
    ) {
        let record = InteractionRecord {
            job_id: job_obj
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            interaction_type: interaction_type.to_string(),
            timestamp: now_ms(),
            title: job_obj
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            company: job_obj
                .get("company")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            url: job_obj
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            source: job_obj
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            location: job_obj
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        app.state::<Mutex<InteractionStore>>()
            .lock()
            .unwrap()
            .upsert(record);
    }
    json!({ "success": true })
}

#[tauri::command]
pub fn scrape_list_postings(app: AppHandle) -> Value {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock().unwrap();
    json!(guard.get_all())
}

#[tauri::command]
pub fn scrape_clear_postings(app: AppHandle) -> Value {
    app.state::<Mutex<PostingsCache>>()
        .lock()
        .unwrap()
        .clear_all();
    json!(null)
}

#[tauri::command]
pub fn scrape_list_interactions(app: AppHandle, filter: Option<Value>) -> Value {
    let filter_type = filter
        .as_ref()
        .and_then(|f| f.get("interactionType"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let binding = app.state::<Mutex<InteractionStore>>();
    let mut store = binding.lock().unwrap();
    json!(store.list(filter_type.as_deref()))
}

#[tauri::command]
pub async fn scrape_export_data(app: AppHandle) -> Value {
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_dialog::FilePath;

    let interactions = {
        let binding = app.state::<Mutex<InteractionStore>>();
        let mut store = binding.lock().unwrap();
        store.export_all()
    };

    let default_name = format!("ajh-export-{}.json", chrono_date());
    let path = app
        .dialog()
        .file()
        .set_title("Export App Data")
        .set_file_name(&default_name)
        .add_filter("JSON", &["json"])
        .blocking_save_file();

    let Some(file_path) = path else {
        return json!({ "success": false });
    };

    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    let bundle = json!({
        "version": 1,
        "exportedAt": now_ms(),
        "interactions": interactions,
    });

    match std::fs::write(&path_str, serde_json::to_string_pretty(&bundle).unwrap_or_default()) {
        Ok(()) => json!({ "success": true, "filePath": path_str }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn scrape_import_data(app: AppHandle) -> Value {
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_dialog::FilePath;

    let path = app
        .dialog()
        .file()
        .set_title("Import App Data")
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    let Some(file_path) = path else {
        return json!({ "success": false, "imported": 0 });
    };

    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    let raw = match std::fs::read_to_string(&path_str) {
        Ok(s) => s,
        Err(e) => return json!({ "success": false, "error": e.to_string(), "imported": 0 }),
    };

    let bundle: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            return json!({ "success": false, "error": "Invalid JSON", "imported": 0 })
        }
    };

    let interactions: Vec<InteractionRecord> = bundle
        .get("interactions")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let imported = app
        .state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .import_bundle(interactions);

    json!({ "success": true, "imported": imported })
}

fn chrono_date() -> String {
    // Simple ISO date without chrono crate.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Approximate: good enough for a filename.
    format!("{days}")
}

// ── Match ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn match_resume(app: AppHandle, req: Value) -> Value {
    let blank = json!({ "ats": 0, "semantic": 0, "combined": 0, "gaps": [], "recommendations": [], "explanation": "" });
    let resume_id = match req.get("resumeId").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return blank,
    };
    let job_text = req.get("jobText").and_then(|v| v.as_str()).unwrap_or("");

    let Ok(store) = app.try_state::<crate::documents::DocumentStore>().ok_or(()) else {
        return blank;
    };
    let resume_vec = match store.get_vector(&resume_id) {
        Some(v) => v,
        None => return blank,
    };
    let job_vec = match crate::documents::embed(job_text).await {
        Some(v) => v,
        None => return blank,
    };
    let semantic = (crate::documents::cosine_similarity(&resume_vec, &job_vec) * 100.0).round() as i64;
    json!({ "ats": semantic, "semantic": semantic, "combined": semantic, "gaps": [], "recommendations": [], "explanation": "" })
}

// ── Credentials ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn credentials_available(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    json!(guard.is_available())
}

#[tauri::command]
pub fn credentials_list(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    json!(guard.list())
}

#[tauri::command]
pub fn credentials_set(app: AppHandle, req: Value) -> Value {
    let board_id = req.get("boardId").and_then(|v| v.as_str()).unwrap_or("");
    let username = req.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let password = req.get("password").and_then(|v| v.as_str()).unwrap_or("");

    if board_id.is_empty() || username.is_empty() || password.is_empty() {
        return json!({ "error": "boardId, username, and password are required" });
    }

    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.set(board_id, username, password) {
        Ok(()) => {
            // Forward to sidecar if it is running.
            let port = app
                .state::<Mutex<ScraperSidecarState>>()
                .lock()
                .ok()
                .and_then(|g| g.port);
            if let Some(port) = port {
                let cmd = serde_json::json!({
                    "kind": "set.credentials",
                    "boardId": board_id,
                    "username": username,
                    "password": password,
                });
                tauri::async_runtime::spawn(async move {
                    reqwest::Client::new()
                        .post(format!("http://127.0.0.1:{port}/command"))
                        .json(&cmd)
                        .send()
                        .await
                        .ok();
                });
            }
            json!({ "success": true })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub fn credentials_remove(app: AppHandle, board_id: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.remove(&board_id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

// ── LinkedIn / Boards ─────────────────────────────────────────────────────────
// Both sets delegate to the sidecar's open.login / board.status / board.disconnect
// commands so the login flow runs in a headed Playwright browser.

async fn sidecar_open_login(app: &AppHandle, board_id: &str) -> Value {
    let Some(port) = sidecar_port(app) else {
        return json!({ "connected": false, "error": "scraper sidecar not ready" });
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "open.login", "boardId": board_id });
    match post_sidecar_command(app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(e) => json!({ "connected": false, "error": e }),
    }
}

async fn sidecar_board_status(app: &AppHandle, board_id: &str) -> Value {
    let Some(port) = sidecar_port(app) else {
        return json!({ "connected": false });
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "board.status", "boardId": board_id });
    match post_sidecar_command(app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(_) => json!({ "connected": false }),
    }
}

async fn sidecar_board_disconnect(app: &AppHandle, board_id: &str) -> Value {
    // Always write { connected: false } to disk so the status survives a sidecar restart.
    write_board_auth_status(board_id, false);

    // Also tell the live sidecar so it clears in-memory state and session cookies.
    if let Some(port) = sidecar_port(app) {
        let job_id = uuid_v4();
        let cmd = json!({ "kind": "board.disconnect", "boardId": board_id });
        post_sidecar_command(app, port, &cmd, &job_id).await.ok();
    }

    json!({ "success": true })
}

/// Resolve the directory the scraper sidecar uses for persistent data.
///
/// Mirrors scraper-runtime/src/index.ts:
///   process.env.AJH_DATA_DIR ?? path.join(os.homedir(), '.ajh')
fn sidecar_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("AJH_DATA_DIR") {
        return std::path::PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    std::path::PathBuf::from(home).join(".ajh")
}

/// Persist the board's connected state directly to the auth-status file on disk.
///
/// The sidecar reads this file on startup to determine whether a board is
/// connected without opening a browser. Writing it from Rust ensures the
/// status survives sidecar restarts even when the sidecar is not running
/// at the time the user clicks Disconnect.
fn write_board_auth_status(board_id: &str, connected: bool) {
    let path = sidecar_data_dir()
        .join("browser-state")
        .join(board_id)
        .join("auth-status.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let payload = if connected { r#"{"connected":true}"# } else { r#"{"connected":false}"# };
    std::fs::write(&path, payload).ok();
}

const KNOWN_BOARDS: &[&str] = &["linkedin", "indeed", "xing", "glassdoor"];

#[tauri::command]
pub async fn linkedin_connect(app: AppHandle) -> Value {
    sidecar_open_login(&app, "linkedin").await
}

#[tauri::command]
pub async fn linkedin_disconnect(app: AppHandle) -> Value {
    sidecar_board_disconnect(&app, "linkedin").await
}

#[tauri::command]
pub async fn linkedin_get_status(app: AppHandle) -> Value {
    sidecar_board_status(&app, "linkedin").await
}

#[tauri::command]
pub async fn boards_connect(app: AppHandle, board_id: String) -> Value {
    sidecar_open_login(&app, &board_id).await
}

#[tauri::command]
pub async fn boards_disconnect(app: AppHandle, board_id: String) -> Value {
    sidecar_board_disconnect(&app, &board_id).await
}

#[tauri::command]
pub async fn boards_get_status(app: AppHandle, board_id: String) -> Value {
    sidecar_board_status(&app, &board_id).await
}

// ── Privacy ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn privacy_sign_out_all(app: AppHandle) -> Value {
    // Disconnect every known board — write auth-status to disk AND tell the sidecar.
    for board_id in KNOWN_BOARDS {
        sidecar_board_disconnect(&app, board_id).await;
    }
    // Clear cached postings and interactions from the in-process stores.
    app.state::<Mutex<PostingsCache>>()
        .lock()
        .unwrap()
        .clear_all();
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .clear_all();
    json!({ "success": true })
}

// ── Apply ────────────────────────────────────────────────────────────────────

/// Start a job application via the scraper sidecar.
///
/// The renderer passes `{ board, url, coverLetter?, bytes?: Uint8Array, name?: string, autoSubmit? }`.
/// Resume bytes (if provided) are base64-encoded before sending to the sidecar,
/// which writes them to a temp file for the Playwright applier.
#[tauri::command]
pub async fn apply_start(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready" });
    };

    let board = req.get("board").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let url = req.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let cover_letter = req.get("coverLetter").and_then(|v| v.as_str()).map(String::from);
    let auto_submit = req.get("autoSubmit").and_then(|v| v.as_bool()).unwrap_or(false);

    // Encode resume bytes as base64 if provided.
    let (resume_bytes_b64, resume_name) = match req.get("bytes") {
        Some(serde_json::Value::Array(arr)) => {
            let bytes: Vec<u8> = arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect();
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let name = req.get("resumeName").and_then(|v| v.as_str()).unwrap_or("resume.pdf").to_string();
            (Some(b64), Some(name))
        }
        _ => (None, None),
    };

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>().lock().unwrap().start(&job_id, "apply.job");

    let payload = serde_json::json!({
        "board": board,
        "url": url,
        "coverLetter": cover_letter,
        "resumeBytesBase64": resume_bytes_b64,
        "resumeName": resume_name,
        "autoSubmit": auto_submit,
    });
    let cmd = json!({ "kind": "apply.job", "jobId": job_id, "payload": payload });

    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(result) => json!({ "jobId": job_id, "result": result }),
        Err(e) => json!({ "error": e, "jobId": job_id }),
    }
}

/// Return the list of boards that support the apply flow.
#[tauri::command]
pub async fn apply_catalog(app: AppHandle) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!([]);
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "apply.catalog" });
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(_) => json!([]),
    }
}

// ── Resume ───────────────────────────────────────────────────────────────────

/// Extract plain text from a resume file (PDF, DOCX, TXT, MD).
///
/// The renderer passes `{ name: string, bytes: Uint8Array }`.
#[tauri::command]
pub async fn resume_extract_text(req: Value) -> Value {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Bytes arrive as a JSON array of integers (Tauri serialises Uint8Array).
    let bytes: Vec<u8> = match req.get("bytes") {
        Some(serde_json::Value::Array(arr)) => {
            arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect()
        }
        _ => return json!({ "error": "bytes field missing or invalid" }),
    };

    match crate::documents::extract_text(&name, &bytes) {
        Ok(text) => json!({ "text": text }),
        Err(e) => json!({ "error": e }),
    }
}

// ── Support ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn support_export_diagnostics() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reload_ai_runtime() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_unload_all_models() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_model_configuration() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_rebuild_vector_indexes() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_embeddings_cache() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_vector_database() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_ocr_cache() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reindex_all_documents() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_all_sessions() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_scraping_queue() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_copy_environment_details(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = format!(
        "AI Job Hunter (Tauri)\nVersion: {}\nShell: tauri\nOS: {} {}\nArch: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        // Reading OS release is impractical from Rust without extra crates;
        // the OS name + arch is the practical diagnostic minimum.
        std::env::consts::FAMILY,
        std::env::consts::ARCH,
    );
    match app.clipboard().write_text(text) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn support_copy_app_version(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    match app.clipboard().write_text(env!("CARGO_PKG_VERSION").to_string()) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn support_copy_system_info(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = format!(
        "OS: {}\nFamily: {}\nArch: {}",
        std::env::consts::OS,
        std::env::consts::FAMILY,
        std::env::consts::ARCH,
    );
    match app.clipboard().write_text(text) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Conversations ────────────────────────────────────────────────────────────

// ── Autopilot ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let list = binding.lock().unwrap().list();
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().get(&autopilot_id);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_create(app: AppHandle, req: Value) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    let ap = store.lock().unwrap().create(req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_update(app: AppHandle, autopilot_id: String, req: Value) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().update(&autopilot_id, req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().remove(&autopilot_id);
    json!(null)
}

#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    let target = {
        let store = app.state::<Mutex<AutopilotStore>>();
        let guard = store.lock().unwrap();
        guard.get(&autopilot_id).map(|ap| ap.target.clone())
    };

    let Some(target) = target else {
        return json!({ "error": format!("autopilot not found: {autopilot_id}") });
    };

    // Proxy to the sidecar as a scrape.board job — the result is tracked
    // via JobTracker so the renderer's job list updates in real time.
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready" });
    };

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "autopilot.run");

    let payload = serde_json::json!({
        "board": target.board,
        "query": target.query,
        "location": target.location,
        "pages": target.pages,
        "dateFilter": target.date_filter,
    });
    let cmd = serde_json::json!({ "kind": "scrape.board", "jobId": job_id, "payload": payload });

    let job_id_ret = job_id.clone();
    tauri::async_runtime::spawn(async move {
        post_sidecar_command(&app, port, &cmd, &job_id).await.ok();
    });

    json!({ "jobId": job_id_ret })
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Active);
    json!(null)
}

// ── Conversations ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn conversations_get_or_create(app: AppHandle) -> Value {
    crate::conversations::get_or_create(&app)
}

#[tauri::command]
pub fn conversations_load_messages(app: AppHandle, conversation_id: String) -> Value {
    crate::conversations::load_messages(&app, &conversation_id)
}

#[tauri::command]
pub fn conversations_save_message(app: AppHandle, req: Value) -> Value {
    crate::conversations::save_message(&app, &req)
}

// ── Native dialogs ────────────────────────────────────────────────────────────

/// Open a native file picker and return the selected file paths.
/// Used by the document import flow instead of passing raw filesystem paths
/// over IPC — the renderer receives paths it can then read via the backend.
///
/// Replaces the Electron `dialog.showOpenDialog` equivalent.
#[tauri::command]
pub async fn dialog_open_files(
    app: AppHandle,
    title: Option<String>,
    filters: Option<Vec<DialogFilter>>,
) -> Vec<String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(&t);
    }
    if let Some(fs) = filters {
        for f in fs {
            builder = builder.add_filter(&f.name, &f.extensions.iter().map(String::as_str).collect::<Vec<_>>());
        }
    }

    builder
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|p| match p {
            FilePath::Path(pb) => pb.to_string_lossy().into_owned(),
            FilePath::Url(u) => u.to_string(),
        })
        .collect()
}

#[derive(serde::Deserialize)]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}
