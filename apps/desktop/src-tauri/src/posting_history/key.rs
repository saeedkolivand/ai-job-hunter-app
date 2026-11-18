//! The ghost key: what "the same posting" means for repost detection.

use crate::scraping::cluster::normalize::{normalize_company, normalize_title};

/// Byte cap on a stored string (CWE-770: scraped text is untrusted).
const MAX_BYTES: usize = 400;

/// `s` cut to [`MAX_BYTES`] on a char boundary.
pub(super) fn clamp(s: &str) -> String {
    let mut end = s.len().min(MAX_BYTES);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Hex chars of the SHA-256 kept in a shortened identity key.
const DIGEST_HEX: usize = 16;

/// [`clamp`] for an IDENTITY key (ghost key, source id): a plain cut would merge
/// two long values sharing a prefix, so an over-cap value becomes a bounded
/// prefix + a digest of the FULL value. Values within the cap, and an already
/// shortened key, come back unchanged.
pub(super) fn clamp_id(s: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    if s.len() <= MAX_BYTES {
        return s.to_string();
    }
    let digest = Sha256::digest(s.as_bytes())
        .iter()
        .take(DIGEST_HEX / 2)
        .fold(String::with_capacity(DIGEST_HEX), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        });
    let mut end = MAX_BYTES - DIGEST_HEX - 1;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}#{digest}", &s[..end])
}

/// Normalized company + title, so a repost under a NEW url (or on another
/// board, or with a `(m/w/d)` suffix) still matches. Deliberately not
/// `canonical_job_key`, which is url-first and would treat every repost as a
/// new job. `None` when either half normalizes to nothing: a key with an empty
/// company would collide across every company.
pub fn ghost_key(company: &str, title: &str) -> Option<String> {
    let company = normalize_company(company);
    let title = normalize_title(title);
    if company.is_empty() || title.is_empty() {
        return None;
    }
    Some(clamp_id(&format!("{company}|{title}")))
}

/// Identity of one scraping source: a board plus the normalized search it ran
/// (`search_key`, built by the call site). Two different searches on one board
/// are different sources: a narrower query not listing a posting says nothing
/// about the wider one.
pub fn source_id(board: &str, search_key: &str) -> String {
    let search = search_key.split_whitespace().collect::<Vec<_>>().join(" ");
    clamp_id(&format!(
        "{}|{}",
        board.trim().to_lowercase(),
        search.to_lowercase()
    ))
}
