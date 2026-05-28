use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsImportRequest {
    pub name: String,
    pub bytes: Vec<u8>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentsEmbedTextRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsUpsertVectorRequest {
    pub doc_id: String,
    pub vector: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentsCosineRequest {
    pub a: Vec<f64>,
    pub b: Vec<f64>,
}

#[tauri::command]
pub async fn documents_list(app: AppHandle) -> Value {
    let Ok(store) = app.try_state::<crate::documents::DocumentStore>().ok_or(()) else {
        return json!([]);
    };
    serde_json::to_value(store.list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn documents_import(app: AppHandle, req: DocumentsImportRequest) -> Value {
    let extraction = match crate::extraction::route(&req.name, &req.bytes) {
        Ok(r) => r,
        Err(crate::extraction::types::ExtractionError::ScannedPdfWithoutOcr) => {
            return json!({ "error": "scanned_pdf", "message": "PDF appears to be scanned. Please upload a text-based PDF or DOCX." });
        }
        Err(e) => return json!({ "error": format!("text extraction failed: {e}") }),
    };
    for w in &extraction.warnings {
        tracing::warn!(warning = %w, file = %req.name, "extraction warning");
    }

    let store = app.state::<crate::documents::DocumentStore>();
    let doc_id = crate::documents::make_doc_id();
    let record = crate::documents::DocumentRecord {
        id: doc_id.clone(),
        title: req.name.clone(),
        name: req.name,
        locale: req.locale,
        text: extraction.text,
        pages: None,
        created_at: crate::documents::now_ms(),
        indexed: false,
        is_default: false,
    };
    match store.insert(&record) {
        Ok(()) => json!({ "id": doc_id, "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn documents_remove(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.remove(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn documents_set_default(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.set_default(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn documents_embed_text(req: DocumentsEmbedTextRequest) -> Value {
    match crate::documents::embed(&req.text).await {
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
pub fn documents_upsert_vector(app: AppHandle, req: DocumentsUpsertVectorRequest) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.upsert_vector(&req.doc_id, &req.vector) {
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
pub fn documents_cosine_similarity(req: DocumentsCosineRequest) -> Value {
    let similarity = crate::documents::cosine_similarity(&req.a, &req.b);
    json!({ "similarity": similarity })
}

#[tauri::command]
pub fn documents_strip_extension(name: String) -> Value {
    json!({ "stripped": crate::documents::strip_extension(&name) })
}
