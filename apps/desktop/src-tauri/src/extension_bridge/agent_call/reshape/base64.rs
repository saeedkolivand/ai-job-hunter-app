//! Re-encoding a raw-byte-array reply field as base64 for the wire — `BASE64_BYTE_FIELDS`'
//! audited `(command, field)` pairs, and the crate-tree-wide byte encoder every payload with a
//! byte-carrying reply uses.

use serde_json::{json, Value};

/// `(command, field)` pairs whose value is a RAW BYTE ARRAY that `serde_json` renders as ~3.2–4×
/// its own size in decimal digits and commas (issue #1138: a one-page résumé PDF came back at
/// 259,841 B, 99.1% of the MCP result cap). Re-encoded base64 (~1.33×) HERE, never on the struct:
/// `ExportResult.data` is the RENDERER's own wire shape too, and `#[serde(with = …)]` would change
/// it for them.
///
/// Audited: `documents_export_document` → `export::types::ExportResult.data: Vec<u8>`.
/// `documents_render_preview_images` is deliberately NOT here — `PreviewResult.pages` is already
/// SVG text, and base64ing it would only make it bigger and unreadable.
pub(in crate::extension_bridge::agent_call) const BASE64_BYTE_FIELDS: &[(&str, &str)] =
    &[("documents_export_document", "data")];

/// Suffix appended to a [`BASE64_BYTE_FIELDS`] field name to form the sibling
/// key that DECLARES the encoding (`data` → `dataEncoding`). Derived from the
/// field name rather than listed per pair so a second entry cannot forget it.
pub(in crate::extension_bridge::agent_call) const ENCODING_KEY_SUFFIX: &str = "Encoding";
pub(in crate::extension_bridge::agent_call) const BASE64_ENCODING: &str = "base64";

/// Re-encode every [`BASE64_BYTE_FIELDS`] array-of-bytes on `command`'s reply as a base64 STRING,
/// and add the sibling `<field>Encoding: "base64"` key that says so — a payload describing its own
/// encoding survives a caller that never read the tool description.
///
/// Top-level only, by exact `(command, field)` pair — the opposite of
/// `fence_named_fields_recursive`'s unconditional walk, on purpose: this is a lossy-looking
/// representation change that must only ever hit the one field whose type was audited, never a
/// "any array of small integers is bytes" heuristic that could rewrite a legitimate score/id array
/// into gibberish.
///
/// A non-array value (or a `data` not entirely bytes) is left as-is with NO marker key, so the two
/// can never disagree.
///
/// Visible to the whole `extension_bridge` tree: the test proving this solves #1138 compares
/// against `agent_cli::mcp::MCP_RESULT_MAX_BYTES`, private to that module — so the test lives
/// THERE, beside the cap, rather than beside a hand-copied literal that could drift.
pub(in crate::extension_bridge) fn base64_byte_fields(command: &str, data: &mut Value) {
    let Some(map) = data.as_object_mut() else {
        return;
    };
    for (cmd, field) in BASE64_BYTE_FIELDS {
        if *cmd != command {
            continue;
        }
        let Some(Value::Array(items)) = map.get(*field) else {
            continue;
        };
        let bytes: Option<Vec<u8>> = items
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect();
        let Some(bytes) = bytes else { continue };
        map.insert((*field).to_string(), json!(encode_base64(&bytes)));
        map.insert(
            format!("{field}{ENCODING_KEY_SUFFIX}"),
            json!(BASE64_ENCODING),
        );
    }
}

/// Base64-encode raw bytes for the wire — the ONE encoder every byte-carrying reply on this
/// bridge uses. `pub(in crate::extension_bridge)` (PR2): `document_export.rs` (a COUSIN of this
/// module, not a descendant) needs it too for `document.export`'s own `data` field, the same
/// reasoning [`base64_byte_fields`] above is visible crate-tree-wide for.
pub(in crate::extension_bridge) fn encode_base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
