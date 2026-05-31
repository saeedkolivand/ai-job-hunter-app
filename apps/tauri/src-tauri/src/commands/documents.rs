use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

// DocumentsImportRequest is generated from DocumentImportRequestSchema by `pnpm gen:ipc`.
pub use crate::ipc_contracts::documents::DocumentsImportRequest;

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

    // Structured review: per-field confidence + missing/low-confidence flags the
    // renderer surfaces before generation. Computed before `extraction.text` is
    // moved into the record.
    let structured = crate::extraction::structured::structure(&extraction);
    let review = serde_json::to_value(&structured).unwrap_or(json!(null));

    // Seed the contact profile from the résumé's links — but ONLY when the user
    // has no profile yet, so an existing (edited) profile is never clobbered. The
    // classifier picks the personal LinkedIn/GitHub/Website by NAME and rejects
    // company / job-board pages; the user reviews & edits it in settings. This is
    // what stops a company link from ever reaching the document header.
    if let Some(cp_store) = app.try_state::<crate::contact_profile::ContactProfileStore>() {
        if cp_store.get().is_effectively_empty() {
            let mut suggested = crate::contact_profile::classify_contact_links(&extraction.links);
            if let Some(email) = structured.email.as_ref().map(|f| f.value.clone()) {
                suggested.email.get_or_insert(email);
            }
            if !suggested.is_effectively_empty() {
                let _ = cp_store.set(&suggested);
            }
        }
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
        Ok(()) => json!({ "id": doc_id, "success": true, "review": review }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Recommend a template + locale from the metadata signals (job title,
/// seniority, top requirements, languages). Rules-based; always overridable.
#[tauri::command]
pub fn documents_recommend_template(req: crate::recommend::RecommendSignals) -> Value {
    serde_json::to_value(crate::recommend::recommend(&req)).unwrap_or(json!(null))
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
pub async fn documents_embed_text(app: AppHandle, req: DocumentsEmbedTextRequest) -> Value {
    match crate::documents::embed(&app, &req.text).await {
        Some(ev) => json!({
            "vector": ev.values,
            "space": { "provider": ev.space.provider, "model": ev.space.model, "dim": ev.space.dim },
        }),
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
    // Externally-supplied vectors are tagged with the active embedding space.
    let cfg = store.embedding_config();
    let ev = crate::commands::ai_provider::EmbeddingVector {
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: cfg.provider,
            model: cfg.model,
            dim: req.vector.len(),
        },
        values: req.vector,
    };
    match store.upsert_vector(&req.doc_id, &ev) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn documents_get_vector(app: AppHandle, doc_id: String) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    match store.get_vector(&doc_id) {
        Some(ev) => json!({
            "vector": ev.values,
            "space": { "provider": ev.space.provider, "model": ev.space.model, "dim": ev.space.dim },
        }),
        None => json!(null),
    }
}

#[tauri::command]
pub fn documents_all_vectors(app: AppHandle) -> Value {
    let store = app.state::<crate::documents::DocumentStore>();
    let out: Vec<Value> = store
        .all_vectors()
        .into_iter()
        .map(|(id, ev)| {
            json!({
                "docId": id,
                "vector": ev.values,
                "space": { "provider": ev.space.provider, "model": ev.space.model, "dim": ev.space.dim },
            })
        })
        .collect();
    json!(out)
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
