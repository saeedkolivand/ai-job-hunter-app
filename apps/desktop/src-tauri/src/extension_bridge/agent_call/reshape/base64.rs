//! Re-encoding a raw-byte-array reply field as base64 for the wire — `BASE64_BYTE_FIELDS`'
//! audited `(command, field)` pairs, and the crate-tree-wide byte encoder every payload with a
//! byte-carrying reply uses.

use serde_json::{json, Value};

/// `(command, field)` pairs whose value is a RAW BYTE ARRAY that `serde_json`
/// renders as ~3.2–4× its own size in decimal digits and commas (issue
/// #1138: an ordinary one-page résumé exported to PDF came back at 259,841 B,
/// 99.1% of the MCP result cap, and a two-page one exceeded it — with no
/// `limit`/`cursor` to narrow and no other exposed export path). Re-encoded
/// base64 (~1.33×) HERE, never on the struct: `ExportResult.data` is the
/// RENDERER's own wire shape (`data: number[]`, consumed by the export
/// service hooks through `AppClient`), and `#[serde(with = …)]` on that field
/// would change it for them too.
///
/// Audited by hand against the struct each pair actually serializes from:
/// - `documents_export_document` → `export::types::ExportResult.data:
///   Vec<u8>` (camelCase-renamed struct; `data` is already its wire key).
///
/// `documents_render_preview_images` is the other payload the MCP cap's own
/// comment names, and it is deliberately NOT here: `PreviewResult.pages` is
/// `Vec<String>` of SVG source, already text, and base64ing it would make it
/// bigger and unreadable.
pub(in crate::extension_bridge::agent_call) const BASE64_BYTE_FIELDS: &[(&str, &str)] =
    &[("documents_export_document", "data")];

/// Suffix appended to a [`BASE64_BYTE_FIELDS`] field name to form the sibling
/// key that DECLARES the encoding (`data` → `dataEncoding`). Derived from the
/// field name rather than listed per pair so a second entry cannot forget it.
pub(in crate::extension_bridge::agent_call) const ENCODING_KEY_SUFFIX: &str = "Encoding";
pub(in crate::extension_bridge::agent_call) const BASE64_ENCODING: &str = "base64";

/// Re-encode every [`BASE64_BYTE_FIELDS`] array-of-bytes on `command`'s reply
/// as a base64 STRING, and add the sibling `<field>Encoding: "base64"` key
/// that says so. A payload that describes its own encoding survives a caller
/// that never read the server `instructions` or the tool description — the
/// reason this is a wire key and not documentation.
///
/// Top-level only, and by exact `(command, field)` pair — the opposite of
/// `fence_named_fields_recursive`'s unconditional recursive walk, on
/// purpose: fencing is a SAFETY property that must cover a field wherever it
/// appears, while this is a lossy-looking representation change that must
/// only ever hit the one field whose type was audited. A recursive
/// "any array of small integers is bytes" rule would eventually rewrite a
/// legitimate array of scores or ids into gibberish.
///
/// A non-array value (or a `data` that is not entirely bytes) is left exactly
/// as it was and gets NO marker key — the marker is only ever added on a
/// value this actually re-encoded, so the two can never disagree.
///
/// Visible to the whole `extension_bridge` tree for ONE reason: the test that
/// proves this actually solves #1138 has to compare against
/// `agent_cli::mcp::MCP_RESULT_MAX_BYTES`, the
/// cap it exists to get under, and that constant is private to the `mcp`
/// module — so the test lives THERE, beside the cap, rather than here beside
/// a hand-copied literal of it that could silently drift (same
/// cross-module-test reasoning as [`gate`]'s own `pub(super)`).
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
