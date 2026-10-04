use regex::Regex;
use std::sync::OnceLock;

use crate::extraction::types::{Confidence, SourceFormat};

/// Score extraction quality based on text content and how it was obtained.
///
/// Direct extraction (PDF text, DOCX, plain) starts at High and can only
/// be downgraded by garbage content. OCR output starts at Medium and is
/// further penalised by garbage.
pub fn score(text: &str, source: SourceFormat) -> Confidence {
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();
    let char_count = text.len();

    // Immediate Low for trivially empty output.
    if word_count < 10 || char_count < 50 {
        return Confidence::Low;
    }

    let garbage_ratio = garbage_ratio(text);
    let has_resume_markers = has_resume_markers(text);

    let ocr_source = matches!(source, SourceFormat::PdfScanned | SourceFormat::Image);

    // Base score from source type.
    let base: i32 = if ocr_source { 1 } else { 2 }; // 0=Low, 1=Medium, 2=High

    // Penalties.
    let penalty: i32 = if garbage_ratio > 0.15 {
        2 // Heavy garbage → always Low
    } else if garbage_ratio > 0.05 {
        1
    } else {
        0
    };

    // Bonus for clear resume signals.
    let bonus: i32 = if has_resume_markers && word_count >= 100 {
        1
    } else {
        0
    };

    match (base - penalty + bonus).clamp(0, 2) {
        0 => Confidence::Low,
        1 => Confidence::Medium,
        _ => Confidence::High,
    }
}

// ── Heuristics ────────────────────────────────────────────────────────────────

fn garbage_ratio(text: &str) -> f64 {
    let total = text.len() as f64;
    if total == 0.0 {
        return 1.0;
    }
    let garbage = text
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !is_common_punctuation(*c))
        .count() as f64;
    garbage / total
}

/// Punctuation (and line breaks) that is ordinary prose, not extraction garbage.
fn is_common_punctuation(c: char) -> bool {
    ".,;:!?-_()[]{}/@#+'\"`~&<>|%*\\\n\r".contains(c)
}

fn has_resume_markers(text: &str) -> bool {
    let lower = text.to_lowercase();

    let has_email = re_email().is_match(text);
    let has_phone = re_phone().is_match(text);

    let section_headers = [
        "experience",
        "education",
        "skills",
        "summary",
        "objective",
        "work history",
        "employment",
        "certifications",
        "languages",
        "projects",
        "references",
        "profile",
        "achievements",
    ];
    let header_matches = section_headers
        .iter()
        .filter(|&&h| lower.contains(h))
        .count();

    (has_email || has_phone) && header_matches >= 2
}

fn re_email() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap())
}

fn re_phone() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[+\(]?[\d\s\-\(\)]{7,}").unwrap())
}

#[cfg(test)]
mod tests;
