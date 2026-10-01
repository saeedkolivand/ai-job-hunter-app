//! resolve_photo unit tests (already covered in photo.rs; re-exercised here through the engine re-export).

use super::fixtures::fixture_photo_data_url;
use crate::export::typst_engine::resolve_photo;

//    integration-layer confidence that the export module re-exports correctly)

#[test]
fn resolve_photo_valid_data_url_returns_png_bytes() {
    let data_url = fixture_photo_data_url();
    let result = resolve_photo(&data_url);
    assert!(
        result.is_some(),
        "resolve_photo must return Some for a valid PNG data URL"
    );
    let bytes = result.unwrap();
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "resolve_photo output must be PNG; got {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn resolve_photo_oversized_returns_none() {
    let huge: String = "A".repeat(15 * 1024 * 1024);
    let data_url = format!("data:image/png;base64,{huge}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "oversized data URL must return None"
    );
}

#[test]
fn resolve_photo_non_image_returns_none() {
    use base64::Engine;
    let garbage = b"not an image at all";
    let b64 = base64::engine::general_purpose::STANDARD.encode(garbage);
    let data_url = format!("data:image/png;base64,{b64}");
    assert!(
        resolve_photo(&data_url).is_none(),
        "non-image bytes must return None"
    );
}

#[test]
fn resolve_photo_bogus_path_returns_none() {
    assert!(
        resolve_photo("/nonexistent/path/photo.png").is_none(),
        "nonexistent path must return None"
    );
}
