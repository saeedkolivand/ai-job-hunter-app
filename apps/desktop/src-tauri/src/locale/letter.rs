//! Cover-letter market conventions for the renderer.
//!
//! The data is the **same** `packages/prompts/src/fixtures/letter-conventions.json`
//! the TS prompt layer uses — embedded at compile time via `include_str!` and
//! parsed once. Sharing the one fixture (rather than a hand-written Rust copy)
//! means the prompt and the renderer can never disagree on a market's salutation,
//! sign-off, subject-line label, or date position. The TS side has its own
//! const + a parity test pinning it to this same file.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

/// One market's letter conventions — mirrors a `markets.<id>` entry in the JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LetterConventions {
    pub country: String,
    pub native_language: String,
    pub formality: String,
    pub length_words: LengthWords,
    /// `"a4"` or `"letter"`.
    pub page: String,
    pub date_format: String,
    /// `"top-right" | "below-header" | "above-salutation"`.
    pub date_position: String,
    pub sender_position: String,
    pub recipient_position: String,
    pub subject_line: SubjectLine,
    pub salutations: Salutations,
    pub signoffs: Vec<String>,
    pub inclusions: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LengthWords {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubjectLine {
    /// `use` is a Rust keyword — the JSON key is `"use"`.
    #[serde(rename = "use")]
    pub used: bool,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Salutations {
    pub named: String,
    pub generic: String,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    markets: HashMap<String, LetterConventions>,
}

/// The embedded fixture, shared verbatim with the TS prompt layer.
const RAW_FIXTURE: &str =
    include_str!("../../../../../packages/prompts/src/fixtures/letter-conventions.json");

static MARKETS: LazyLock<HashMap<String, LetterConventions>> = LazyLock::new(|| {
    serde_json::from_str::<Fixture>(RAW_FIXTURE)
        .expect("parse letter-conventions.json (shared fixture)")
        .markets
});

/// Letter conventions for a market id (case-insensitive), falling back to the
/// international baseline so an unknown/blank market always renders cleanly.
pub fn conventions(market: &str) -> &'static LetterConventions {
    let key = market.trim().to_lowercase();
    MARKETS
        .get(&key)
        .or_else(|| MARKETS.get("intl"))
        .expect("intl market always present in the fixture")
}

// ─── Parser detection helpers ─────────────────────────────────────────────────
//
// The renderer parses a finished letter whose market it may not know, so these
// recognize salutation / sign-off / subject lines across every supported locale
// (a superset of the fixture's native forms plus the common letter-language
// forms). They are intentionally permissive — a missed match degrades to a body
// line, never a crash.

/// Leading words that begin a salutation, lower-cased, across all locales.
const SALUTATION_PREFIXES: &[&str] = &[
    // en
    "dear",
    "to whom it may concern",
    // de
    "sehr geehrte",
    "sehr geehrter",
    // fr
    "madame",
    "monsieur",
    // es
    "estimado",
    "estimada",
    // it
    "gentile",
    "egregio",
    "spettabile",
    // pt / br
    "exmo",
    "exma",
    "caro",
    "cara",
    "prezado",
    "prezada",
    // tr
    "sayın",
    // ru
    "уважаем",
    "здравствуйте",
    // zh
    "尊敬的",
    // ja
    "採用ご担当",
    "拝啓",
    // ko
    "채용 담당자",
    "안녕하",
];

/// Leading words that begin a sign-off, lower-cased, across all locales.
const SIGNOFF_PREFIXES: &[&str] = &[
    // en
    "sincerely",
    "best regards",
    "best,",
    "kind regards",
    "warm regards",
    "regards",
    "yours sincerely",
    "yours faithfully",
    // de
    "mit freundlichen",
    "mit freundlichem",
    "hochachtungsvoll",
    "viele grüße",
    "viele grüsse",
    "beste grüße",
    // fr
    "cordialement",
    "je vous prie",
    "sincères salutations",
    "veuillez agréer",
    // es
    "atentamente",
    "saludos cordiales",
    "un cordial saludo",
    // it
    "distinti saluti",
    "cordiali saluti",
    "in attesa",
    // pt / br
    "com os melhores",
    "atenciosamente",
    "cordialmente",
    "cumprimentos",
    // tr
    "saygılarımla",
    "iyi çalışmalar",
    // ru
    "с уважением",
    // zh
    "此致",
    // ja
    "よろしくお願い",
    "敬具",
    "敬白",
    // ko
    "감사합니다",
];

/// Subject-line keywords (lower-cased); a subject line is one of these followed
/// by a colon (`:` or full-width `：`), optionally with spaces in between.
const SUBJECT_KEYWORDS: &[&str] = &[
    "betreff", "subject", "re", "objet", "oggetto", "asunto", "assunto", "konu", "тема", "主题",
    "件名", "제목",
];

/// True when a line opens a salutation in any supported locale.
pub fn is_salutation(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    // Korean salutations begin with the recipient's name, so match the honorific.
    if lower.contains("님께") {
        return true;
    }
    SALUTATION_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// True when a line opens a sign-off in any supported locale.
pub fn is_signoff(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    SIGNOFF_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// True when a line is a subject line (e.g. `Betreff: …`, `Objet : …`, `件名：…`).
pub fn is_subject_line(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    SUBJECT_KEYWORDS.iter().any(|kw| {
        lower.strip_prefix(kw).is_some_and(|rest| {
            let r = rest.trim_start();
            r.starts_with(':') || r.starts_with('：')
        })
    })
}

/// Literal template-placeholder tokens across the markets this app supports
/// (en + de at minimum) — lower-cased for comparison against a
/// trailing-punctuation-trimmed line.
const PLACEHOLDER_TOKENS: &[&str] = &[
    "ihr name",
    "ihr vollständiger name",
    "your name",
    "your full name",
    "name",
    "ihre position",
    "your title",
    "your position",
    "[name]",
    "vorname nachname",
    "first last",
];

/// True when `line` is an unfilled template-placeholder slot rather than real
/// content — either whole-line bracket/slot syntax (`[...]`, `<...>`,
/// `{...}`, `{{...}}`) or one of [`PLACEHOLDER_TOKENS`] (case-insensitive,
/// trailing-punctuation tolerant, mirroring the candidate-name comparison
/// style used around the cover-letter parser's signature-title detection).
///
/// A model trained on résumé/letter templates can reproduce an unfilled slot
/// verbatim (e.g. German "Ihr Name") even though the prompt tells it not to —
/// see ADR-034 Consequence #2. This is the shared predicate both the letter
/// parser (`export/typst_engine/letter.rs`, so the placeholder is never
/// promoted to `signature_title`) and content validation
/// (`validate/content/letter.rs`, `letter.template_placeholder`) use, so the
/// pattern list exists in exactly one place.
pub fn is_template_placeholder(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let slot_wrapped = (trimmed.starts_with('[') && trimmed.ends_with(']'))
        || (trimmed.starts_with('<') && trimmed.ends_with('>'))
        || (trimmed.starts_with('{') && trimmed.ends_with('}'));
    if slot_wrapped {
        return true;
    }
    let normalized = trimmed.trim_end_matches([',', '.']).trim().to_lowercase();
    PLACEHOLDER_TOKENS.contains(&normalized.as_str())
}

#[cfg(test)]
mod tests;
