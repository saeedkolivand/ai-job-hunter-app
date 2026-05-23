use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

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

    let bytes = {
        use base64::Engine;
        match base64::engine::general_purpose::STANDARD.decode(&bytes_b64) {
            Ok(b) => b,
            Err(e) => return json!({ "error": format!("base64 decode: {e}") }),
        }
    };

    // Extract text from the document
    let text = match crate::documents::extract_text(&name, &bytes) {
        Ok(t) => t,
        Err(e) => return json!({ "error": format!("text extraction failed: {e}") }),
    };

    let store = app.state::<crate::documents::DocumentStore>();
    let doc_id = crate::documents::make_doc_id();
    let record = crate::documents::DocumentRecord {
        id: doc_id.clone(),
        title: name.clone(),
        name: name.clone(),
        locale,
        text,
        pages: None,
        created_at: crate::documents::now_ms(),
        indexed: false,
    };
    match store.insert(&record) {
        Ok(()) => json!({ "id": doc_id, "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn documents_delete(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.remove(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn documents_embed_text(req: Value) -> Value {
    let text = req.get("text").and_then(|v| v.as_str()).unwrap_or("");
    match crate::documents::embed(text).await {
        Some(vector) => json!({ "vector": vector }),
        None => json!({ "error": "embedding failed" }),
    }
}

#[tauri::command]
pub fn documents_set_indexed(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.set_indexed(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn documents_upsert_vector(app: AppHandle, req: Value) -> Value {
    let doc_id = req.get("docId").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let vector: Vec<f64> = req.get("vector")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default();
    
    let store = app.state::<crate::documents::DocumentStore>();
    match store.upsert_vector(&doc_id, &vector) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn documents_get_vector(app: AppHandle, doc_id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.get_vector(&doc_id) {
        Some(vector) => json!({ "vector": vector }),
        None => json!(null),
    }
}

#[tauri::command]
pub fn documents_all_vectors(app: AppHandle) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    json!(store.all_vectors())
}

#[tauri::command]
pub fn documents_cosine_similarity(req: Value) -> Value {
    let a: Vec<f64> = req.get("a")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default();
    let b: Vec<f64> = req.get("b")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default();
    
    let similarity = crate::documents::cosine_similarity(&a, &b);
    json!({ "similarity": similarity })
}

#[tauri::command]
pub fn documents_strip_extension(name: String) -> Value {
    json!({ "stripped": crate::documents::strip_extension(&name) })
}
