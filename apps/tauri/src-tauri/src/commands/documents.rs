use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::documents::keywords::keywords_normalized;
use crate::error::AppResult;

// DocumentsImportRequest is generated from DocumentImportRequestSchema by `pnpm gen:ipc`.
pub use crate::ipc_contracts::documents::DocumentsImportRequest;

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

    // Seed/complete the contact profile from the résumé. `classify_contact_links`
    // picks the personal LinkedIn/GitHub/Website by NAME (rejecting company /
    // job-board pages) and keeps every other personal link as a labelled extra;
    // email / phone / location come from the deterministic structuring pass. We
    // MERGE into the existing profile, filling only empty fields — so a profile the
    // user edited is never clobbered, but a sparse one (e.g. website-only) is
    // completed with the résumé's email / phone / location / extra links. The
    // header builds from these named fields, which is what keeps a company link
    // out of the document header.
    // Conflicts between the imported contact and the saved profile (both sides
    // non-empty, normalized values differ). The import NEVER blocks on these — it
    // still silently fills empty fields below — but they (plus the full extracted
    // contact) are returned so the renderer can let the user resolve each one. If
    // there is no contact-profile state, conflicts are empty and `suggested` null.
    let mut contact_conflicts: Vec<crate::contact_profile::ContactFieldConflict> = Vec::new();
    let mut suggested_contact: Value = json!(null);
    if let Some(cp_store) = app.try_state::<crate::contact_profile::ContactProfileStore>() {
        let mut suggested = crate::contact_profile::classify_contact_links(&extraction.links);
        if let Some(email) = structured.email.as_ref().map(|f| f.value.clone()) {
            suggested.email.get_or_insert(email);
        }
        if let Some(phone) = structured.phone.as_ref().map(|f| f.value.clone()) {
            suggested.phone.get_or_insert(phone);
        }
        if let Some(loc) = structured.location.as_ref().map(|f| f.value.clone()) {
            if !loc.trim().is_empty() {
                suggested
                    .location
                    .get_or_insert_with(|| crate::contact_profile::LocalizedText {
                        default: loc,
                        ..Default::default()
                    });
            }
        }
        let current = cp_store.get();
        contact_conflicts = crate::contact_profile::detect_contact_conflicts(&current, &suggested);
        suggested_contact = serde_json::to_value(&suggested).unwrap_or(json!(null));
        if !suggested.is_effectively_empty() {
            // Empty fields still fill silently; conflicting (already-set) fields
            // are preserved and reported above for the user to resolve.
            let mut merged = current.clone();
            merged.fill_empty_from(&suggested);
            if let Err(e) = cp_store.set(&merged) {
                // Import still succeeds; only the autofill persist failed. Log the
                // error WITHOUT any contact values (no email/phone/name/url — PII).
                tracing::warn!(error = %e, "failed to persist autofilled contact profile on import");
            }
        }
    }
    let conflicts_json = serde_json::to_value(&contact_conflicts).unwrap_or(json!([]));

    let store = app.state::<crate::documents::DocumentStore>();
    let doc_id = crate::documents::make_doc_id();
    // Cache the normalized (un-stemmed) keyword set so the match path skips
    // re-tokenizing this résumé. Sorted for deterministic JSON; stemming is
    // applied at match time so the cache stays language-agnostic.
    let mut kw_vec: Vec<String> = keywords_normalized(&extraction.text).into_iter().collect();
    kw_vec.sort();
    let keywords_json = serde_json::to_string(&kw_vec).ok();
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
        keywords_json,
    };
    match store.insert(&record) {
        Ok(()) => json!({
            "id": doc_id,
            "success": true,
            "review": review,
            "contactConflicts": conflicts_json,
            "suggestedContact": suggested_contact,
        }),
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

/// Fetch the stored extracted text for a single document by id. Returns an
/// empty string when the document is missing (the renderer treats "no text" and
/// "no document" the same — it only ever seeds a generator with this), so this
/// never errors on a missing row. `AppResult` is the typed-command convention.
#[tauri::command]
pub async fn documents_get_text(app: AppHandle, id: String) -> AppResult<String> {
    let store = app.state::<crate::documents::DocumentStore>();
    Ok(store.get(&id).map(|doc| doc.text).unwrap_or_default())
}
