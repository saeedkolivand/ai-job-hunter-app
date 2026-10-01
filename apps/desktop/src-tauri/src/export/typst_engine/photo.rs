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
mod tests;
