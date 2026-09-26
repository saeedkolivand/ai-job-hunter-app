//! Tests for the raw-byte-array base64 re-encode (`reshape/base64.rs`).

use super::super::super::agent_cli::policy::Effect;
use super::super::policy_lookup::find_policy;
use super::super::reshape::*;
use super::super::*;

/// The audited const pinned against a hand-written literal, same reasoning as
/// the paging list above, plus the row's own policy check.
#[test]
fn the_base64_byte_fields_are_exactly_this_one_audited_pair() {
    assert_eq!(BASE64_BYTE_FIELDS, &[("documents_export_document", "data")]);
    let entry = find_policy("commands", "documents_export_document")
        .expect("documents_export_document is a real POLICY row");
    assert_eq!(entry.effect, Effect::Read);
}

/// The other half of that pair — the FIELD name — pinned against the struct
/// it was audited against rather than against a second copy of the literal.
/// `BASE64_BYTE_FIELDS` names `data` from memory of
/// `export::types::ExportResult`; rename that field (or put a
/// `#[serde(rename)]` on it) and every assertion above still passes while the
/// pair silently addresses a key no reply carries — i.e. the raw byte array
/// #1138 exists to shrink ships unencoded. So serialize the REAL struct here,
/// prove the audited name is the key holding its bytes, and run the re-encode
/// on that exact value.
#[test]
fn the_audited_field_is_the_key_the_real_export_struct_serializes_its_bytes_under() {
    let (command, field) = BASE64_BYTE_FIELDS[0];
    let mut value = serde_json::to_value(crate::export::types::ExportResult {
        data: vec![0x25, 0x50, 0x44, 0x46],
        mime_type: "application/pdf".to_string(),
        filename: "resume.pdf".to_string(),
        report: None,
    })
    .expect("ExportResult serializes");

    let bytes = value
        .get(field)
        .unwrap_or_else(|| {
            panic!(
                "`{field}` is no longer a key of ExportResult's wire shape — \
             BASE64_BYTE_FIELDS now points at nothing: {value}"
            )
        })
        .as_array()
        .unwrap_or_else(|| panic!("`{field}` is no longer serialized as an array: {value}"));
    assert!(
        bytes.iter().all(|b| b.as_u64().is_some_and(|n| n <= 255)),
        "`{field}` must be the RAW byte array this re-encodes: {value}"
    );

    base64_byte_fields(command, &mut value);
    assert_eq!(value[field].as_str().unwrap(), "JVBERg==", "%PDF, base64'd");
    let marker = format!("{field}{ENCODING_KEY_SUFFIX}");
    assert_eq!(value[&marker].as_str().unwrap(), BASE64_ENCODING);
}

#[test]
fn base64_byte_fields_encodes_the_export_bytes_and_marks_the_encoding() {
    let mut data = json!({
        "data": [80, 68, 70, 45],
        "mimeType": "application/pdf",
        "filename": "resume.pdf",
    });
    base64_byte_fields("documents_export_document", &mut data);

    assert_eq!(data["data"].as_str().unwrap(), "UERGLQ==");
    // A self-describing payload: a caller that never read the tool
    // description still learns the encoding from the reply itself.
    assert_eq!(data["dataEncoding"].as_str().unwrap(), BASE64_ENCODING);
    // Every sibling field untouched.
    assert_eq!(data["mimeType"].as_str().unwrap(), "application/pdf");
    assert_eq!(data["filename"].as_str().unwrap(), "resume.pdf");
}

/// The guard's other direction — mutation-check the `(command, field)` pair:
/// the IDENTICAL payload under a different command name must come back
/// byte-for-byte unchanged, with no marker key. Deleting the `*cmd !=
/// command` check makes this fail.
#[test]
fn base64_byte_fields_leaves_every_other_command_untouched() {
    let original = json!({ "data": [80, 68, 70, 45], "scores": [1, 2, 3] });
    let mut data = original.clone();
    base64_byte_fields("documents_render_preview_images", &mut data);
    assert_eq!(data, original);

    // And on the RIGHT command, an unlisted field is still untouched — the
    // pair is `(command, field)`, not "every array on a matching command".
    let mut same_command = original.clone();
    base64_byte_fields("documents_export_document", &mut same_command);
    assert_eq!(same_command["scores"], original["scores"]);
}

/// A value that isn't an array of bytes is left alone AND gets no marker —
/// the marker is only ever added to something this actually re-encoded, so
/// the two can never disagree.
#[test]
fn base64_byte_fields_never_marks_a_value_it_did_not_re_encode() {
    for odd in [json!("already a string"), json!([1, 2, 999]), json!(null)] {
        let mut data = json!({ "data": odd.clone() });
        base64_byte_fields("documents_export_document", &mut data);
        assert_eq!(data["data"], odd);
        assert!(
            data.get("dataEncoding").is_none(),
            "no marker without a re-encode: {odd}"
        );
    }
}

// ── Drop dead fields (issue #1171's residual, `B1-r3-ACLI-R7-3`) ──────────
