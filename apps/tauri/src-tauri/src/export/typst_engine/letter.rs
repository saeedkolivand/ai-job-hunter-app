//! Cover-letter structured model + parser for the Typst rendering engine.
//!
//! [`parse_cover_letter`] splits a finished letter text into letterhead /
//! date / recipient / subject / salutation / body / signoff / signature,
//! reusing [`crate::locale::letter`] detection helpers so the engine
//! recognises all supported markets and salutations.
//!
//! **Injection safety**: [`LetterModel`] is serialised to JSON and injected
//! via the virtual `data.json` — no user content is ever concatenated into
//! Typst markup.
//!
//! Offline hard-wall: this file contains NO typst / typst_pdf imports.
//! All Typst types remain inside `engine.rs` and `render.rs`.

use serde::Serialize;

use crate::contact_profile::ContactProfile;
use crate::locale::letter::conventions;
use crate::model::rich::{tokenize_rich, TextRun};

// ── Serialisable rich-text run (mirrors render::JsonTextRun) ──────────────────
//
// Duplicated here so the letter module is self-contained and does NOT depend
// on `render.rs` types (avoids circular module dependencies while keeping the
// JSON shape identical).

#[derive(Debug, Clone, Serialize)]
pub(super) struct LetterRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl From<&TextRun> for LetterRun {
    fn from(r: &TextRun) -> Self {
        Self {
            text: r.text.clone(),
            bold: r.bold,
            italic: r.italic,
            link: r.link.clone(),
        }
    }
}

// ── LetterModel ───────────────────────────────────────────────────────────────

/// Structured cover-letter model ready for JSON serialisation into `data.json`.
///
/// All fields that the template may not need are `Option`; only `body` and
/// `opts` are always present. Missing parts degrade gracefully — the template
/// guards every optional key with `"k" in d`.
#[derive(Debug, Serialize)]
pub(super) struct LetterModel {
    pub opts: LetterOpts,
    pub style: LetterStyle,
    pub letterhead: LetterHead,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub recipient_lines: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salutation: Option<String>,
    /// Body paragraphs as rich-text runs so **bold** survives.
    pub body: Vec<Vec<LetterRun>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signoff: Option<String>,
    pub signature_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_title: Option<String>,
}

/// Page + locale options passed through to the template.
#[derive(Debug, Serialize)]
pub(super) struct LetterOpts {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub lang: String,
    /// `"top-right"` | `"below-header"` | `"above-salutation"`.
    pub date_position: String,
    /// `"top"` | `"bottom"`.
    pub sender_position: String,
    /// `"before-date"` | `"after-date"`.
    pub recipient_position: String,
    pub subject_line_used: bool,
    pub subject_line_label: String,
}

/// Letterhead block: candidate name + tokenised contact runs.
#[derive(Debug, Serialize)]
pub(super) struct LetterHead {
    pub name: String,
    /// Rich-text runs for the contact line (links first-class).
    pub contact: Vec<LetterRun>,
}

/// Styling pulled from the chosen resume [`Template`] so the letter visually
/// matches the resume family.
#[derive(Debug, Serialize)]
pub(super) struct LetterStyle {
    /// Validated accent hex (`#RRGGBB`).
    pub c_accent: String,
    /// Body colour hex.
    pub c_body: String,
    /// Name colour hex.
    pub c_name: String,
    /// Date / muted colour hex.
    pub c_date: String,
    /// Rule colour hex.
    pub c_rule: String,
    /// Typst font family name for the heading / name.
    pub font_name: String,
    /// Typst font family name for the body.
    pub font_body: String,
    pub name_pt: f32,
    pub body_pt: f32,
}

// ── Page geometry constants ───────────────────────────────────────────────────

/// A4 dimensions in mm.
pub(super) const A4_W: f32 = 210.0;
pub(super) const A4_H: f32 = 297.0;
/// US Letter dimensions in mm.
pub(super) const LETTER_W: f32 = 215.9;
pub(super) const LETTER_H: f32 = 279.4;

// ── Parser ────────────────────────────────────────────────────────────────────

/// Resolve page geometry from the market's page field (`"a4"` or `"letter"`).
pub(super) fn page_dims(market: &str) -> (f32, f32) {
    let conv = conventions(market);
    if conv.page == "letter" {
        (LETTER_W, LETTER_H)
    } else {
        (A4_W, A4_H)
    }
}

/// Lazy date-pattern regex — matches month names or 4-digit years.
fn looks_like_date(s: &str) -> bool {
    // Matches lines that contain digits and common date separators, e.g.:
    //   "June 2, 2025" / "2. Juni 2025" / "02/06/2025" / "2025-06-02"
    //   "2 juin 2025" / "le 2 juin 2025"
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    if !has_digit {
        return false;
    }
    // Must contain a year-like 4-digit run or a separator ( / . - space)
    // alongside a digit to distinguish from plain phone numbers or IDs.
    let has_year = t.split_whitespace().any(|w| {
        let digits: String = w.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.len() == 4
    });
    let has_sep = t.contains('/') || t.contains('.') || t.contains('-');
    has_year || (has_digit && has_sep)
}

/// Heuristic: is this line a contact line that should be skipped in the body
/// (email address, phone number, URL, or pipe-separated items)?
fn looks_like_contact_line(s: &str) -> bool {
    let t = s.trim();
    t.contains('@')
        || t.contains("http://")
        || t.contains("https://")
        || (t.contains('|') && t.len() > 4)
        || (t.contains('·') && t.len() > 4)
}

/// Strip leading markdown bold/italic markers (`**`, `*`, `__`) from a line so
/// that a subject line rendered bold in the text (`**Betreff: …**`) is still
/// detected by `is_subject_line`.
fn strip_leading_md_emphasis(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("**").unwrap_or(s);
    let s = s.strip_prefix("__").unwrap_or(s);
    s.strip_prefix('*').unwrap_or(s)
}

/// Parse a finished cover-letter text into a structured [`LetterModel`].
///
/// Parsing rules:
/// - Skips leading header lines (name + contact line echoed from the text).
/// - Recognises salutations/sign-offs via `locale::letter` helpers.
/// - Pre-salutation lines: subject (`is_subject_line`), date
///   (`looks_like_date`), or recipient.
/// - Body paragraphs are parsed as rich-text runs via `tokenize_rich` so
///   **bold** phrases survive.
/// - Post-signoff: first non-blank non-name line is signature_title.
///
/// Gracefully handles all-missing parts — body may be empty but never panics.
pub(super) fn parse_cover_letter(
    text: &str,
    contact: Option<&ContactProfile>,
    meta_name: Option<&str>,
    market: &str,
    lang: &str,
    style: LetterStyle,
) -> LetterModel {
    let conv = conventions(market);

    // Page geometry from market convention.
    let (page_w, page_h) = page_dims(market);

    let opts = LetterOpts {
        page_width_mm: page_w,
        page_height_mm: page_h,
        lang: lang.to_string(),
        date_position: conv.date_position.clone(),
        sender_position: conv.sender_position.clone(),
        recipient_position: conv.recipient_position.clone(),
        subject_line_used: conv.subject_line.used,
        subject_line_label: conv.subject_line.label.clone(),
    };

    // ── Letterhead ────────────────────────────────────────────────────────────
    // Name: prefer generation metadata, then first non-blank text line.
    let raw_lines: Vec<&str> = text.lines().collect();

    let name_text: String = meta_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| {
            raw_lines
                .iter()
                .map(|l| l.trim())
                .find(|l| !l.is_empty())
                .unwrap_or("")
                .to_string()
        });

    // Contact line: use named profile fields (shared with the resume header);
    // fall back to scraping the letter text only when no profile is supplied.
    let contact_md: String = match contact {
        Some(profile) if !profile.is_effectively_empty() => profile.header_markdown(lang),
        _ => raw_lines
            .iter()
            .take(4)
            .map(|l| l.trim())
            .filter(|l| looks_like_contact_line(l))
            .map(|l| {
                // strip_md equivalent: remove markdown links to get plain text
                // for the fallback plain-text contact line
                l.to_string()
            })
            .collect::<Vec<_>>()
            .join(" | "),
    };

    let contact_runs: Vec<LetterRun> = if contact_md.is_empty() {
        Vec::new()
    } else {
        tokenize_rich(&contact_md)
            .iter()
            .map(LetterRun::from)
            .collect()
    };

    let letterhead = LetterHead {
        name: name_text.clone(),
        contact: contact_runs,
    };

    // ── Body parsing ──────────────────────────────────────────────────────────
    let mut body_started = false;
    let mut skip_lines = 0usize;
    let mut paragraphs: Vec<Vec<LetterRun>> = Vec::new();
    let mut current_para_text = String::new();
    let mut date_str: Option<String> = None;
    let mut recipient_lines: Vec<String> = Vec::new();
    let mut subject_line: Option<String> = None;
    let mut salutation_line: Option<String> = None;
    let mut closing_line: Option<String> = None;
    let mut after_closing = false;
    let mut signature_title: Option<String> = None;

    for raw_line in &raw_lines {
        let trimmed = raw_line.trim();
        // Clean: strip markdown links to plain text for detection only.
        // We keep `trimmed` (raw) for rich-text body parsing.
        let clean = strip_md_links(trimmed);

        // Skip lines that were part of the header (name + blank + contact echo).
        if skip_lines < 3
            && (clean.is_empty()
                || trimmed == name_text.as_str()
                || (!contact_md.is_empty() && contact_md.contains(clean.trim())))
        {
            skip_lines += 1;
            continue;
        }

        // Drop any stray contact line the generated text still carries before
        // the body — the letterhead already renders the contact from the profile.
        if !body_started && looks_like_contact_line(&clean) {
            continue;
        }

        // Empty line → paragraph break.
        if clean.is_empty() {
            if !current_para_text.is_empty() {
                paragraphs.push(
                    tokenize_rich(&current_para_text)
                        .iter()
                        .map(LetterRun::from)
                        .collect(),
                );
                current_para_text.clear();
            }
            continue;
        }

        // For detection, also strip leading emphasis markers that the prompt
        // may emit around subject lines.
        let clean_for_detect = strip_leading_md_emphasis(&clean);

        let is_salutation = crate::locale::letter::is_salutation(clean_for_detect);
        let is_signoff = crate::locale::letter::is_signoff(clean_for_detect);

        if is_salutation {
            if !current_para_text.is_empty() {
                paragraphs.push(
                    tokenize_rich(&current_para_text)
                        .iter()
                        .map(LetterRun::from)
                        .collect(),
                );
                current_para_text.clear();
            }
            salutation_line = Some(clean_for_detect.to_string());
            body_started = true;
            continue;
        }

        if is_signoff {
            if !current_para_text.is_empty() {
                paragraphs.push(
                    tokenize_rich(&current_para_text)
                        .iter()
                        .map(LetterRun::from)
                        .collect(),
                );
                current_para_text.clear();
            }
            closing_line = Some(clean_for_detect.to_string());
            after_closing = true;
            continue;
        }

        if after_closing {
            // First non-blank line after closing that is not the candidate name
            // is the signature title (e.g. "Software Engineer").
            if signature_title.is_none() && !clean.is_empty() && clean.trim() != name_text.as_str()
            {
                signature_title = Some(clean.to_string());
            }
            continue;
        }

        if !body_started {
            // Pre-salutation: subject, date, or recipient.
            if crate::locale::letter::is_subject_line(clean_for_detect) && subject_line.is_none() {
                subject_line = Some(clean_for_detect.to_string());
            } else if looks_like_date(&clean) && date_str.is_none() {
                date_str = Some(clean.to_string());
            } else {
                recipient_lines.push(clean.to_string());
            }
        } else {
            // Body: accumulate into current paragraph (space-join across lines).
            if !current_para_text.is_empty() {
                current_para_text.push(' ');
            }
            current_para_text.push_str(trimmed);
        }
    }

    // Flush any trailing paragraph.
    if !current_para_text.is_empty() {
        paragraphs.push(
            tokenize_rich(&current_para_text)
                .iter()
                .map(LetterRun::from)
                .collect(),
        );
    }

    // Signature name: prefer generation metadata, then the stored name.
    let signature_name = meta_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| name_text.clone());

    LetterModel {
        opts,
        style,
        letterhead,
        date: date_str,
        recipient_lines,
        subject: subject_line,
        salutation: salutation_line,
        body: paragraphs,
        signoff: closing_line,
        signature_name,
        signature_title,
    }
}

/// Strip markdown link syntax `[label](url)` → `label` for plain-text
/// detection (does NOT affect rich-text rendering — use `tokenize_rich` for
/// that). Also strips `**bold**` markers for content comparison.
fn strip_md_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            // Collect label up to `]`
            let mut label = String::new();
            let mut closed = false;
            for ch in chars.by_ref() {
                if ch == ']' {
                    closed = true;
                    break;
                }
                label.push(ch);
            }
            if closed {
                // Check for `(url)`
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume `(`
                    for ch in chars.by_ref() {
                        if ch == ')' {
                            break;
                        }
                    }
                }
                out.push_str(&label);
            } else {
                out.push('[');
                out.push_str(&label);
            }
        } else if c == '*' {
            // Skip markdown bold/italic markers
            if chars.peek() == Some(&'*') {
                chars.next();
            }
        } else if c == '_' {
            if chars.peek() == Some(&'_') {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── Style builder ─────────────────────────────────────────────────────────────

/// Build a [`LetterStyle`] from the [`Template`] registry entry so the letter
/// visually matches the chosen resume family.
pub(super) fn style_from_template(t: &crate::export::templates::Template) -> LetterStyle {
    use crate::export::types::FontFamily;

    fn rgb_hex(r: u8, g: u8, b: u8) -> String {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }

    fn font_name(f: FontFamily) -> &'static str {
        match f {
            FontFamily::Calibri => "Carlito",
            FontFamily::Inter => "Inter",
            FontFamily::SourceSerif4 => "Source Serif 4",
            FontFamily::Manrope => "Manrope",
        }
    }

    LetterStyle {
        c_accent: rgb_hex(t.accent_color.0, t.accent_color.1, t.accent_color.2),
        c_body: rgb_hex(t.body_color.0, t.body_color.1, t.body_color.2),
        c_name: rgb_hex(t.name_color.0, t.name_color.1, t.name_color.2),
        c_date: rgb_hex(t.date_color.0, t.date_color.1, t.date_color.2),
        c_rule: rgb_hex(t.rule_color.0, t.rule_color.1, t.rule_color.2),
        font_name: font_name(t.fonts.name_family).to_string(),
        font_body: font_name(t.fonts.body_family).to_string(),
        name_pt: t.name_pt,
        body_pt: t.body_pt,
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_style() -> LetterStyle {
        LetterStyle {
            c_accent: "#2563EB".to_string(),
            c_body: "#222222".to_string(),
            c_name: "#111111".to_string(),
            c_date: "#555555".to_string(),
            c_rule: "#aaaaaa".to_string(),
            font_name: "Carlito".to_string(),
            font_body: "Carlito".to_string(),
            name_pt: 20.0,
            body_pt: 10.5,
        }
    }

    const EN_LETTER: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my interest in the Software Engineer position at Acme Corp. \
I have five years of experience building distributed systems.

During my time at Beta Inc, I led the migration of our payments service, reducing \
latency by 40 percent and cutting costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";

    const DE_LETTER: &str = "\
Max Müller
max@example.de | https://linkedin.com/in/maxmueller

Frankfurt, 2. Juni 2025

Frau Dr. Anna Weber
Musterfirma GmbH
Hauptstraße 1
60311 Frankfurt am Main

Betreff: Bewerbung als Software Engineer

Sehr geehrte Frau Dr. Weber,

mit großem Interesse habe ich Ihre Stellenausschreibung gelesen und bewerbe mich \
hiermit um die Position als Software Engineer.

In meiner bisherigen Tätigkeit bei der Beta GmbH konnte ich umfangreiche Erfahrungen \
in der Entwicklung verteilter Systeme sammeln.

Über eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen.

Mit freundlichen Grüßen,

Max Müller
";

    #[test]
    fn parses_english_letter_all_fields() {
        let model = parse_cover_letter(
            EN_LETTER,
            None,
            Some("Jane Smith"),
            "us",
            "en",
            dummy_style(),
        );

        assert_eq!(model.letterhead.name, "Jane Smith");
        assert!(
            model.date.as_deref().unwrap_or("").contains("2025"),
            "date should be captured; got {:?}",
            model.date
        );
        assert!(
            !model.recipient_lines.is_empty(),
            "recipient block should be present"
        );
        assert!(
            model.recipient_lines.iter().any(|l| l.contains("Acme")),
            "recipient lines should include company name"
        );
        assert!(
            model.subject.is_none(),
            "US market letter should have no subject line"
        );
        assert!(
            model
                .salutation
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .starts_with("dear"),
            "salutation should start with 'Dear'; got {:?}",
            model.salutation
        );
        assert!(!model.body.is_empty(), "body paragraphs must not be empty");
        assert!(
            model
                .signoff
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .starts_with("sincerely"),
            "signoff should be 'Sincerely'; got {:?}",
            model.signoff
        );
        assert_eq!(model.signature_name, "Jane Smith");
        assert!(
            model
                .signature_title
                .as_deref()
                .unwrap_or("")
                .contains("Engineer"),
            "signature title should be captured; got {:?}",
            model.signature_title
        );
    }

    #[test]
    fn parses_german_letter_subject_line() {
        let model = parse_cover_letter(
            DE_LETTER,
            None,
            Some("Max Müller"),
            "de",
            "de",
            dummy_style(),
        );

        assert_eq!(model.letterhead.name, "Max Müller");
        assert!(
            model.date.is_some(),
            "DE letter must have a date; got {:?}",
            model.date
        );
        assert!(
            !model.recipient_lines.is_empty(),
            "recipient block should be present"
        );
        assert!(
            model.subject.is_some(),
            "DE letter must have a subject line"
        );
        let subject = model.subject.as_deref().unwrap();
        assert!(
            subject.to_lowercase().contains("betreff"),
            "subject must contain 'Betreff'; got {subject:?}"
        );
        let sal = model.salutation.as_deref().unwrap_or("");
        assert!(
            sal.to_lowercase().starts_with("sehr geehr"),
            "German salutation not detected; got {sal:?}"
        );
        assert!(
            model
                .signoff
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("freundlichen"),
            "German signoff not detected; got {:?}",
            model.signoff
        );
    }

    #[test]
    fn de_market_uses_a4_us_market_uses_letter() {
        let m_de = parse_cover_letter("x", None, None, "de", "de", dummy_style());
        assert_eq!(m_de.opts.page_width_mm, A4_W);
        assert_eq!(m_de.opts.page_height_mm, A4_H);

        let m_us = parse_cover_letter("x", None, None, "us", "en", dummy_style());
        assert_eq!(m_us.opts.page_width_mm, LETTER_W);
        assert_eq!(m_us.opts.page_height_mm, LETTER_H);
    }

    #[test]
    fn de_market_opts_carry_subject_label_and_date_position() {
        let m = parse_cover_letter("x", None, None, "de", "de", dummy_style());
        assert!(m.opts.subject_line_used);
        assert_eq!(m.opts.subject_line_label, "Betreff");
        assert_eq!(m.opts.date_position, "top-right");
    }

    #[test]
    fn body_rich_text_preserves_bold() {
        let letter = "\
Alice
alice@example.com

Jan 1, 2025

Hiring Manager
FooCo

Dear Hiring Manager,

I **significantly** improved our pipeline. Results were **outstanding**.

Sincerely,
Alice
";
        let model = parse_cover_letter(letter, None, Some("Alice"), "us", "en", dummy_style());
        assert!(!model.body.is_empty(), "body must not be empty");
        // At least one run in the body should be bold
        let has_bold = model.body.iter().any(|para| para.iter().any(|r| r.bold));
        assert!(has_bold, "bold runs should survive in body paragraphs");
    }

    #[test]
    fn strip_md_links_removes_link_syntax() {
        let input = "Check out [GitHub](https://github.com/user) and [LinkedIn](https://linkedin.com/in/user).";
        let result = strip_md_links(input);
        assert!(result.contains("GitHub"), "label should remain");
        assert!(result.contains("LinkedIn"), "label should remain");
        assert!(!result.contains("https://"), "URL should be stripped");
    }

    #[test]
    fn looks_like_date_recognises_common_formats() {
        assert!(looks_like_date("June 2, 2025"));
        assert!(looks_like_date("2. Juni 2025"));
        assert!(looks_like_date("02/06/2025"));
        assert!(looks_like_date("2025-06-02"));
        assert!(!looks_like_date("Dear Hiring Manager,"));
        assert!(!looks_like_date("Acme Corp"));
    }

    #[test]
    fn unknown_market_falls_back_gracefully() {
        // Should not panic; intl baseline applies
        let model = parse_cover_letter("x", None, None, "zz", "en", dummy_style());
        assert_eq!(model.opts.page_width_mm, A4_W); // intl uses A4
    }
}
