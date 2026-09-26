//! Shared fixture for `proof::tests`' topic modules — a real `DocumentRecord`, needed by the
//! `extract_list_match`, `fencing_a` and `fencing_b` topics alike.

use serde_json::Value;

/// A real `DocumentRecord` fixture (HIGH fix — security review round 2),
/// per the finding's own instruction: "build the test fixture from
/// `serde_json::to_value(DocumentRecord{..})` rather than a hand-typed
/// literal — a literal is what let this pass." `DocumentRecord` renames
/// its id to `_id` on the wire; the two `documents_list`-backed
/// `ListMatch` rows (`documents_remove`, `resume_pipeline_run`) were
/// matching on `"id"`, which a real response never has, so every attempt
/// resolved `proof_unavailable` forever.
pub(super) fn a_document_record(id: &str, name: &str) -> Value {
    serde_json::to_value(crate::documents::DocumentRecord {
        id: id.to_string(),
        title: "Resume".to_string(),
        name: name.to_string(),
        locale: None,
        text: "…".to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    })
    .unwrap()
}
