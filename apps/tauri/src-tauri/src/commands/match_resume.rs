use serde_json::{json, Value};

#[tauri::command]
pub async fn match_resume(_req: Value) -> Value {
    // Stub - implement when needed
    json!(null)
}

#[tauri::command]
pub async fn resume_extract_text(req: Value) -> Value {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let bytes: Vec<u8> = match req.get("bytes") {
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect(),
        _ => return json!({ "error": "bytes field missing or invalid" }),
    };

    match crate::extraction::route(&name, &bytes) {
        Ok(r) => json!({ "text": r.text, "confidence": format!("{:?}", r.confidence) }),
        Err(crate::extraction::types::ExtractionError::ScannedPdfWithoutOcr) => {
            json!({ "error": "scanned_pdf", "message": "PDF appears to be scanned. Please upload a text-based PDF or DOCX." })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}
