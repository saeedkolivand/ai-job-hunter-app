//! Tests for resolve_photo: valid/oversized/non-image/path-rejection inputs + downscaling.

use super::*;
use base64::Engine;
use image::{ImageBuffer, Rgba};

/// Generate a small solid-color RGBA PNG and return its raw bytes.
fn solid_png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
    let img = ImageBuffer::from_fn(w, h, |_, _| color);
    let dynamic = DynamicImage::ImageRgba8(img);
    let mut buf = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("test: encode png");
    buf
}

/// Build a `data:image/png;base64,<payload>` data URL from raw PNG bytes.
fn to_data_url(png: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    format!("data:image/png;base64,{b64}")
}

// ── (1) Valid PNG data URL → Some(png_bytes) ──────────────────────────────

#[test]
fn valid_png_data_url_resolves_to_some() {
    let png = solid_png(240, 240, Rgba([200u8, 100, 50, 255]));
    let data_url = to_data_url(&png);

    let result = resolve_photo(&data_url);
    assert!(
        result.is_some(),
        "resolve_photo should return Some for a valid PNG data URL"
    );
    let bytes = result.unwrap();
    // Output must be valid PNG.
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "output must be a PNG; got {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

// ── (2) Output is PNG (re-encoded) ────────────────────────────────────────

#[test]
fn output_is_always_png() {
    // Send in a JPEG-encoded image via data URL to confirm re-encode to PNG.
    let mut jpeg_buf = Vec::new();
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(60, 60, |_, _| {
        Rgba([10u8, 20, 30, 255])
    }))
    .write_to(&mut Cursor::new(&mut jpeg_buf), ImageFormat::Jpeg)
    .unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_buf);
    let data_url = format!("data:image/jpeg;base64,{b64}");

    let result = resolve_photo(&data_url);
    assert!(result.is_some(), "JPEG data URL should resolve to Some");
    let bytes = result.unwrap();
    assert!(
        bytes.starts_with(b"\x89PNG"),
        "output must be re-encoded as PNG even when input was JPEG"
    );
    // Must not be the original JPEG bytes.
    assert_ne!(
        bytes, jpeg_buf,
        "output must differ from the raw JPEG input"
    );
}

// ── (3) Oversized input (>10 MB) → None ──────────────────────────────────

#[test]
fn oversized_data_url_returns_none() {
    // Build a string that claims to be a valid data URL but with >14 MB of
    // base64 payload (simulates an oversized input).
    let huge_b64: String = "A".repeat(15 * 1024 * 1024); // 15 MB base64 characters
    let data_url = format!("data:image/png;base64,{huge_b64}");

    let result = resolve_photo(&data_url);
    assert!(
        result.is_none(),
        "oversized data URL should return None, not OOM"
    );
}

// ── (4) Non-image bytes → None ────────────────────────────────────────────

#[test]
fn non_image_bytes_returns_none() {
    // A plain text file disguised as PNG.
    let garbage = b"This is definitely not a PNG image file at all.";
    let b64 = base64::engine::general_purpose::STANDARD.encode(garbage);
    let data_url = format!("data:image/png;base64,{b64}");

    let result = resolve_photo(&data_url);
    assert!(
        result.is_none(),
        "non-image bytes should return None, not a crash"
    );
}

// ── (5) Path traversal / file-path inputs → None ─────────────────────────
//
// These inputs must ALL return None.  The file-path code path has been
// removed entirely; any non-data-URI input is rejected at the top of
// resolve_photo before touching the filesystem.

#[test]
fn relative_path_traversal_returns_none() {
    assert!(
        resolve_photo("../../etc/passwd").is_none(),
        "relative path traversal must return None"
    );
}

#[test]
fn unix_absolute_path_returns_none() {
    assert!(
        resolve_photo("/etc/passwd").is_none(),
        "absolute Unix path must return None"
    );
}

#[test]
fn windows_absolute_path_returns_none() {
    assert!(
        resolve_photo(r"C:\Windows\System32\drivers\etc\hosts").is_none(),
        "absolute Windows path must return None"
    );
}

#[test]
fn bare_filename_returns_none() {
    assert!(
        resolve_photo("photo.png").is_none(),
        "bare filename with no scheme must return None"
    );
}

#[test]
fn empty_string_returns_none() {
    assert!(resolve_photo("").is_none(), "empty string must return None");
    assert!(
        resolve_photo("   ").is_none(),
        "whitespace-only string must return None"
    );
}

// ── (6) Unknown MIME type in data URL → None ──────────────────────────────

#[test]
fn svg_data_url_returns_none() {
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg'></svg>";
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg);
    let data_url = format!("data:image/svg+xml;base64,{b64}");

    let result = resolve_photo(&data_url);
    assert!(
        result.is_none(),
        "SVG data URL should return None (SVG is rejected)"
    );
}

// ── (7) Large image is downscaled ─────────────────────────────────────────

#[test]
fn large_image_is_downscaled_to_max_edge() {
    // 2000×1500 solid image — longer edge is 2000, which exceeds MAX_EDGE_PX (1200).
    let png = solid_png(2000, 1500, Rgba([128u8, 64, 32, 255]));
    let data_url = to_data_url(&png);

    let result = resolve_photo(&data_url);
    assert!(result.is_some(), "large image should resolve to Some");

    let bytes = result.unwrap();
    // Decode the output and check dimensions.
    let out_img = image::load_from_memory(&bytes).expect("output must be valid image");
    let (out_w, out_h) = (out_img.width(), out_img.height());
    let longest = out_w.max(out_h);
    assert!(
        longest <= MAX_EDGE_PX,
        "downscaled longest edge {longest} exceeds MAX_EDGE_PX ({MAX_EDGE_PX})"
    );
    // Aspect ratio should be preserved approximately (allow ±2 px rounding).
    let aspect_orig = 2000.0_f64 / 1500.0;
    let aspect_out = out_w as f64 / out_h as f64;
    let delta = (aspect_orig - aspect_out).abs();
    assert!(
        delta < 0.02,
        "aspect ratio changed too much: orig={aspect_orig:.3} out={aspect_out:.3}"
    );
}
