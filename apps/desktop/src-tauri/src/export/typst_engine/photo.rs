//! Safe render-side photo loading for the Typst engine.
//!
//! [`resolve_photo`] converts a raw `ContactProfile.photo` string into clean,
//! sanitised PNG bytes suitable for embedding in a Typst document via the
//! virtual file `/photo.png`.  It returns `None` on ANY problem — the templates
//! must always handle the no-photo case gracefully.
//!
//! Security contract:
//! - Only `data:image/<mime>;base64,<payload>` URIs are accepted.  File paths
//!   are rejected unconditionally — there is no legitimate use-case for reading
//!   an arbitrary path from IPC.
//! - Raw input is capped at 10 MB before any decoding (prevents zip-bomb /
//!   large-data-URL OOM attacks).
//! - Only raster images are accepted (PNG / JPEG via explicit `image` crate
//!   format detection); SVG, EXR, HDR, etc. are rejected.
//! - Decoded image dimensions are capped: the longest edge is downscaled to at
//!   most 1200 px (a résumé photo needs at most ~300 px; 1200 is very generous).
//! - Output is always re-encoded as lossless PNG.  This strips all EXIF/XMP/ICC
//!   metadata — a privacy win — and produces a deterministic canonical form.
//! - All errors are swallowed; the function never panics and never surfaces an
//!   error type to the caller.

use image::{DynamicImage, ImageFormat, ImageReader};
use std::io::Cursor;

/// Maximum raw input size (before base64 decode): 10 MB.
const MAX_RAW_BYTES: usize = 10 * 1024 * 1024;

/// Maximum longest edge in pixels for the decoded image.  Images larger than
/// this are downscaled with Lanczos3 before re-encoding.
const MAX_EDGE_PX: u32 = 1200;

/// Resolve a raw `ContactProfile.photo` value to sanitised PNG bytes, or `None`.
///
/// Accepts ONLY `data:image/<mime>;base64,<payload>` URIs where `<mime>` is one
/// of `png`, `jpeg`, `jpg`, `webp`, or `gif`.  Any other input — including
/// absolute file paths, relative paths, bare filenames, empty strings, or
/// unrecognised schemes — is rejected and returns `None`.
///
/// After decoding the image is optionally downscaled (if the longest edge
/// exceeds `MAX_EDGE_PX`) and then re-encoded to PNG.  The output strips all
/// metadata (EXIF/XMP/ICC) automatically via the `image` crate's encode path.
///
/// Returns `None` on ANY problem: bad format, oversized input, unrecognised
/// MIME type, non-image bytes.  Never panics.
pub fn resolve_photo(raw: &str) -> Option<Vec<u8>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // Only data: URIs are accepted.  Anything that is not a data:image/ URI
    // (including file paths, bare names, http URLs, etc.) → None.
    let rest = raw.strip_prefix("data:image/")?;
    let raw_bytes = decode_data_url(rest)?;

    // Cap raw bytes BEFORE any further decode (defence-in-depth even though
    // decode_data_url already caps before returning).
    if raw_bytes.len() > MAX_RAW_BYTES {
        return None;
    }

    decode_and_sanitise(&raw_bytes)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Decode a `data:image/<rest>` URL where `rest` is everything after
/// `data:image/`.  Accepts only recognised raster MIME types.
fn decode_data_url(rest: &str) -> Option<Vec<u8>> {
    // rest = "<mime-suffix>;base64,<payload>"
    let (mime_suffix, payload) = rest.split_once(";base64,")?;

    // Restrict to safe raster MIME types only.
    match mime_suffix.to_lowercase().as_str() {
        "png" | "jpeg" | "jpg" | "webp" | "gif" => {}
        _ => return None,
    }

    // Cap the base64 payload length: base64-encoded data is ~133% of binary;
    // 10 MB binary → max ~13.4 MB base64.  We cap the b64 string itself at
    // 14 MB as a round upper bound.
    if payload.len() > 14 * 1024 * 1024 {
        return None;
    }

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;

    if bytes.len() > MAX_RAW_BYTES {
        return None;
    }

    Some(bytes)
}

/// Decode raw bytes as a raster image using the `image` crate (explicit format
/// detection), optionally downscale, then re-encode to PNG.
fn decode_and_sanitise(raw: &[u8]) -> Option<Vec<u8>> {
    // Use a `Cursor` so we avoid any file-system access.
    let reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .ok()?;

    // Reject formats that are not the MIME-gated raster types (png/jpeg/webp/gif).
    // BMP and TIFF are intentionally excluded: the upstream MIME gate in
    // `decode_data_url` never admits them, so this branch is dead — keeping it
    // here would be misleading and could create a gap if the MIME list diverges.
    match reader.format() {
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif) => {}
        _ => return None,
    }

    let img: DynamicImage = reader.decode().ok()?;

    // Downscale if the longest edge exceeds the cap.
    let img = downscale_if_needed(img);

    // Re-encode to PNG (strips all EXIF/XMP/ICC metadata).
    let mut out: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .ok()?;

    Some(out)
}

/// Downscale `img` so its longest edge is at most `MAX_EDGE_PX`, preserving
/// aspect ratio via Lanczos3.  Returns the original image if already within bounds.
fn downscale_if_needed(img: DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);
    if longest <= MAX_EDGE_PX {
        return img;
    }
    // Scale factor: longest-edge / MAX_EDGE_PX, applied to both dimensions.
    let new_w = ((w as f64 * MAX_EDGE_PX as f64 / longest as f64).round()) as u32;
    let new_h = ((h as f64 * MAX_EDGE_PX as f64 / longest as f64).round()) as u32;
    let new_w = new_w.max(1);
    let new_h = new_h.max(1);
    img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
}
