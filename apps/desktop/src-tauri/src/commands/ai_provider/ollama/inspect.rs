//! Ollama `/api/show` model inspection + `/api/pull` streamed model
//! download. Split out of `ollama.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, JobEvent, JOBS_EVENT};

use super::super::{map_completion_transport_error, timeouts};
use super::host;

/// Inspect a local model via `/api/show` — its real trained context length and
/// size labels — normalized to the `ModelInspectResult` shape. Returns
/// `Value::Null` when Ollama is unreachable, errors, or returns nothing useful,
/// so the caller can surface "no info" without failing.
pub async fn show_model(model: &str) -> Value {
    let body = json!({ "model": model });
    let resp = match crate::net::http::shared()
        .post(format!("{}/api/show", host()))
        .timeout(timeouts::OLLAMA_SHOW)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Value::Null,
    };
    if !resp.status().is_success() {
        return Value::Null;
    }
    match crate::net::http::read_json_capped::<Value>(
        resp,
        crate::net::http::DEFAULT_MAX_BODY_BYTES,
    )
    .await
    {
        Ok(data) => normalize_show(&data),
        Err(_) => Value::Null,
    }
}

/// Map an Ollama `/api/show` response to the `ModelInspectResult` shape
/// (camelCase keys), omitting fields the server didn't provide. Pure +
/// unit-tested. `model_info.*.context_length` is keyed by architecture (e.g.
/// `llama.context_length`, `qwen2.context_length`), so we scan for the first key
/// ending in `.context_length` rather than hardcoding an architecture. Returns
/// `Value::Null` when nothing usable is present.
pub(super) fn normalize_show(data: &Value) -> Value {
    let context_length = data
        .get("model_info")
        .and_then(|mi| mi.as_object())
        .and_then(|obj| {
            obj.iter()
                .find(|(k, _)| k.ends_with(".context_length"))
                .and_then(|(_, v)| v.as_u64())
        });
    let details = data.get("details");
    let str_field = |key: &str| -> Option<String> {
        details
            .and_then(|d| d.get(key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    let mut out = serde_json::Map::new();
    if let Some(c) = context_length {
        out.insert("contextLength".to_string(), json!(c));
    }
    if let Some(p) = str_field("parameter_size") {
        out.insert("parameterSize".to_string(), json!(p));
    }
    if let Some(q) = str_field("quantization_level") {
        out.insert("quantization".to_string(), json!(q));
    }
    if let Some(f) = str_field("family") {
        out.insert("family".to_string(), json!(f));
    }

    if out.is_empty() {
        Value::Null
    } else {
        Value::Object(out)
    }
}

/// Stream a model pull, emitting `jobs:event` progress. Returns when complete.
pub async fn pull(app: &AppHandle, job_id: &str, model: &str) -> AppResult<()> {
    let mut response = crate::net::http::shared()
        .post(format!("{}/api/pull", host()))
        .timeout(timeouts::MODEL_PULL)
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| map_completion_transport_error(e, "Ollama", timeouts::MODEL_PULL))?;

    if !response.status().is_success() {
        let status = response.status();
        let body =
            crate::net::http::read_text_capped(response, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(AppError::Provider(format!(
            "Ollama {status}: {}",
            crate::commands::ai_provider::redact_upstream_text(&body)
        )));
    }

    let mut line_buf = String::new();
    // Bytes from a read that ended mid-UTF-8-sequence — see `stream::push_utf8`.
    let mut carry: Vec<u8> = Vec::new();
    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
        super::super::stream::push_utf8(&mut line_buf, &mut carry, &bytes);
        // Walk by a `consumed` offset and drain the parsed prefix once after the
        // inner loop, instead of reallocating the whole tail per line (O(n²)).
        let mut consumed = 0;
        while let Some(rel) = line_buf[consumed..].find('\n') {
            let nl = consumed + rel;
            let line = line_buf[consumed..nl].trim().to_string();
            consumed = nl + 1;
            if line.is_empty() {
                continue;
            }
            let event: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let status = event.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let completed = event
                .get("completed")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let total = event.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let digest = event.get("digest").and_then(|v| v.as_str()).unwrap_or("");
            let p = if total > 0.0 { completed / total } else { 0.0 };
            emit_event(
                app,
                JOBS_EVENT,
                JobEvent {
                    r#type: "job.stream".to_string(),
                    job_id: job_id.to_string(),
                    data: Some(
                        json!({ "status": status, "p": p, "completed": completed, "total": total, "digest": digest }),
                    ),
                    ts: crate::db::now_ms() as i64,
                },
            );
            if status == "success" {
                return Ok(());
            }
        }
        // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
        line_buf.drain(..consumed);
    }
    Ok(())
}
