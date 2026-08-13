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
    /// Horizontal-placement hint from the shared fixture (`"left"` | `"top-right"`),
    /// passed through verbatim. Not currently read by any `.typ` template — every
    /// letter layout (Classic/Refined/Banded) renders the recipient block
    /// unconditionally, left-aligned, right after the date. (fr's true top-right
    /// recipient layout is not implemented — a real top-right variant is a
    /// possible follow-up; kept in the data contract for that future use.)
    pub recipient_position: String,
    pub subject_line_used: bool,
    pub subject_line_label: String,
    /// ATS mode — the request's `ats_mode`, threaded through so a layout can
    /// drop its **decorative, non-semantic** elements (Sidebar's tinted rail,
    /// Monogram's initials device, Banded's accent band) while keeping every
    /// word of the letter.
    ///
    /// This is the letter-side counterpart of the résumé's `RenderOpts.ats` and
    /// `theme::has_header_band`: "both renderers drop it in ATS mode". Layouts
    /// gate on THIS, never on the layout id — the id picks the arrangement, the
    /// opts decide which parts of it are legal for this export.
    pub ats: bool,
}

/// Letterhead block: candidate name + tokenised contact runs.
#[derive(Debug, Serialize)]
pub(super) struct LetterHead {
    pub name: String,
    /// Rich-text runs for the contact line (links first-class).
    pub contact: Vec<LetterRun>,
    /// Up to two uppercase initials derived from [`Self::name`] — see
    /// [`monogram_initials`]. Part of the shared model (any layout may use it);
    /// only `letter_monogram.typ` renders it today, and only outside ATS mode.
    pub initials: String,
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

/// Prefer the contact profile's casing for an ALL-CAPS name.
///
/// When `name` is ALL-CAPS (has uppercase, no lowercase) and the profile's
/// `full_name` matches it case-insensitively, adopt the profile's casing so the
/// letterhead and signature don't render shouted (e.g. a stored "SAEED KOLIVAND"
/// becomes the profile's "Saeed Kolivand"). Pure pass-through otherwise: no
/// profile, a different-name profile, or an already mixed-case name are all
/// returned unchanged — never title-cased (so "MCDONALD" / "O'BRIEN" are safe).
fn prefer_profile_casing(name: String, contact: Option<&ContactProfile>) -> String {
    let all_caps = name.chars().any(char::is_uppercase) && !name.chars().any(char::is_lowercase);
    if !all_caps {
        return name;
    }
    match contact.and_then(|p| p.full_name.as_deref()) {
        Some(full) if full.trim().to_lowercase() == name.trim().to_lowercase() => {
            full.trim().to_string()
        }
        _ => name,
    }
}

/// Up to **two** uppercase initials for a letterhead monogram device: the
/// initial of the first NAME token and the initial of the last one.
///
/// Derived in Rust rather than in Typst because every interesting case is string
/// handling a `.typ` cannot be tested on: a mononym ("Prince" → `P`), a
/// multi-part surname ("Jane van der Berg" → `JB`, first + LAST, not the first
/// two), and non-ASCII capitals ("Àlvaro Èsposito" → `ÀÈ`, which must survive
/// PDF extraction like every other accented capital in this engine).
///
/// Two kinds of token are **not** names and are dropped:
///
/// 1. Anything that does not START with a LETTER — a pronoun parenthetical,
///    "—", a stray bullet, or a number. The first version searched each token
///    for its first alphanumeric *anywhere*, so "(they/them)" contributed a
///    `T`; "Jane Smith (they/them)" came out `JT`. Alphanumeric was still too
///    loose: an initial is never a digit, so "12 March 2025" offered `12`
///    as a monogram. Letters only.
/// 2. A token that abbreviates a WORD — two or more letters before its period
///    ("Dr.", "Prof.", "Dipl.-Ing.", "Ph.D."). Those are titles and
///    qualifications; "Dr. Jane Smith" is `JS`, and the German
///    "Dipl.-Ing. Max Müller" is `MM`, not `DM`. A SINGLE-letter initial keeps
///    its period and still counts, so "J. Smith" is `JS` rather than `S`.
///
/// Never longer than two characters, so the device is a fixed-size square no
/// matter how long the name is — a third initial would overflow it. Returns an
/// empty string for a nameless letterhead; the template skips the device then.
fn monogram_initials(name: &str) -> String {
    /// Is this token a person's name, as opposed to punctuation, a number or a
    /// title?
    fn is_name_token(tok: &str) -> bool {
        if !tok.starts_with(char::is_alphabetic) {
            return false;
        }
        // Letters before the first period: 1 is an initial ("J."), 2+ is a
        // word abbreviation ("Dr.", "Dipl.-Ing."). No period at all → a name.
        match tok.split_once('.') {
            Some((head, _)) => head.chars().count() < 2,
            None => true,
        }
    }

    let mut initials = name
        .split_whitespace()
        .filter(|tok| is_name_token(tok))
        // Guaranteed `Some` — `is_name_token` required a leading alphanumeric.
        .filter_map(|tok| tok.chars().next());

    let Some(first) = initials.next() else {
        return String::new();
    };
    // `to_uppercase` can expand (ß → SS); take one char so the device stays
    // exactly one glyph per initial.
    let up = |c: char| c.to_uppercase().next().unwrap_or(c);

    // `next_back`, not `last`: the iterator is double-ended, and `first` has
    // already been consumed, so this is the last REMAINING token — a mononym
    // therefore yields `None` here rather than re-finding its own initial.
    match initials.next_back() {
        Some(last) => [up(first), up(last)].iter().collect(),
        None => up(first).to_string(),
    }
}

/// Initials for the letterhead device, or empty when the "name" is not a name.
///
/// The letterhead name falls back to the first non-blank LINE of the letter when
/// no candidate name is supplied — and three renderer call sites pass an empty
/// `candidate_name`, so that fallback is reachable in production. On a
/// letterhead-less letter the first line is the salutation, which made the
/// device read `DM`, from "Dear Hiring Manager,".
///
/// A DATE is the fourth opening this has to refuse, and the one both formats
/// missed: a letter whose first line is "12 March 2025" put `12` in the device.
/// The DOCX line filter excluded salutation/sign-off/subject and nothing else,
/// so it had the identical hole.
///
/// **Both renderers call THIS function** — the DOCX path used to call
/// [`monogram_initials`] directly, which is how the two drifted in the first
/// place. One guard, one place; a fifth opening kind gets added once.
///
/// Scoped to the DEVICE deliberately: what `letterhead.name` itself should show
/// for a letterhead-less letter is a separate, pre-existing question, and
/// widening this would change all six layouts' output.
pub(crate) fn letterhead_initials(name_text: &str) -> String {
    use crate::locale::letter::{is_salutation, is_signoff, is_subject_line};
    if is_salutation(name_text)
        || is_signoff(name_text)
        || is_subject_line(name_text)
        || looks_like_date(name_text)
    {
        return String::new();
    }
    monogram_initials(name_text)
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
///
/// `ats` is the request's ATS mode. It is a required parameter rather than a
/// default so a new caller has to state it: an ATS toggle that silently fails to
/// reach the renderer looks identical to one that works.
pub(super) fn parse_cover_letter(
    text: &str,
    contact: Option<&ContactProfile>,
    meta_name: Option<&str>,
    market: &str,
    lang: &str,
    style: LetterStyle,
    ats: bool,
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
        ats,
    };

    // ── Letterhead ────────────────────────────────────────────────────────────
    // Name: prefer generation metadata, then first non-blank text line.
    let raw_lines: Vec<&str> = text.lines().collect();

    // B.2: an ALL-CAPS stored name adopts the contact profile's mixed-case
    // casing (when it matches case-insensitively) so the letterhead + signature
    // don't render shouted; no-op with no profile or a different-name profile.
    let name_text: String = prefer_profile_casing(
        meta_name
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| {
                raw_lines
                    .iter()
                    .map(|l| l.trim())
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
                    .to_string()
            }),
        contact,
    );

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
        initials: letterhead_initials(&name_text),
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

    // B.1: case-insensitive header-name compare, computed from the final
    // (possibly re-cased) `name_text`, so the applicant's own name — however the
    // model cased it — is skipped rather than landing in the recipient block.
    let name_lower = name_text.trim().to_lowercase();

    for raw_line in &raw_lines {
        let trimmed = raw_line.trim();
        // Clean: strip markdown links to plain text for detection only.
        // We keep `trimmed` (raw) for rich-text body parsing.
        let clean = strip_md_links(trimmed);

        // Skip lines that were part of the header (name + blank + contact echo).
        //
        // Bounded to the PRE-BODY zone by `!body_started`: `skip_lines` only
        // advances when this arm actually fires, so on a letter with fewer than
        // three leading header lines the arm stayed armed into the body and ate
        // its blank lines — swallowing the paragraph breaks (the `clean.is_empty()`
        // flush below never ran) and space-joining the whole letter into one
        // run-on block. Safe: `current_para_text` is only ever appended to in the
        // `body_started` branch, so nothing can be pending while this is reachable.
        if !body_started
            && skip_lines < 3
            && (clean.is_empty()
                || clean.trim().trim_end_matches([',', '.']).to_lowercase() == name_lower
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
            //
            // Case-insensitive + trailing-punctuation-tolerant comparison so that
            // an LLM sign-off in title-case ("Saeed Kolivand") is still recognised
            // as the candidate name even when `name_text` is stored uppercase
            // ("SAEED KOLIVAND") — preventing the name from being promoted to
            // `signature_title` and rendered a second time.
            let is_candidate_name = clean.trim().trim_end_matches([',', '.']).to_lowercase()
                == name_text.trim().to_lowercase();
            if signature_title.is_none() && !clean.is_empty() && !is_candidate_name {
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

    // Signature name equals the letterhead name verbatim: both derive from
    // `meta_name` (or the first text line) with profile casing preferred (B.2),
    // so the signoff never renders a different casing than the letterhead.
    let signature_name = name_text.clone();

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
    fn body_paragraphs_survive_a_letter_with_no_letterhead() {
        // The agent's `draft_cover_letter` prompt asks for the finished letter
        // only — "no preamble" — so the draft opens straight at the salutation
        // with no name/contact echo to consume the three header skips. The skip
        // arm then ate the BODY's blank lines and the whole letter collapsed into
        // a single run-on paragraph.
        let letter = "Dear Hiring Manager,\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n\nSincerely,\n\nJane Smith\n";
        let model = parse_cover_letter(
            letter,
            None,
            Some("Jane Smith"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(
            model.body.len(),
            3,
            "each blank-line-separated paragraph must stay separate; got {:?}",
            model.body
        );
        assert_eq!(model.salutation.as_deref(), Some("Dear Hiring Manager,"));
    }

    #[test]
    fn body_paragraphs_survive_a_letter_with_only_a_date_and_salutation() {
        // Two leading pre-body lines instead of three — still not enough to burn
        // the skip budget before the body starts.
        let letter = "12 March 2025\n\nDear Hiring Manager,\n\nFirst paragraph.\n\nSecond paragraph.\n\nSincerely,\n\nJane Smith\n";
        let model = parse_cover_letter(
            letter,
            None,
            Some("Jane Smith"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(model.body.len(), 2, "got {:?}", model.body);
    }

    #[test]
    fn parses_english_letter_all_fields() {
        let model = parse_cover_letter(
            EN_LETTER,
            None,
            Some("Jane Smith"),
            "us",
            "en",
            dummy_style(),
            false,
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
            false,
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
        let m_de = parse_cover_letter("x", None, None, "de", "de", dummy_style(), false);
        assert_eq!(m_de.opts.page_width_mm, A4_W);
        assert_eq!(m_de.opts.page_height_mm, A4_H);

        let m_us = parse_cover_letter("x", None, None, "us", "en", dummy_style(), false);
        assert_eq!(m_us.opts.page_width_mm, LETTER_W);
        assert_eq!(m_us.opts.page_height_mm, LETTER_H);
    }

    #[test]
    fn de_market_opts_carry_subject_label_and_date_position() {
        let m = parse_cover_letter("x", None, None, "de", "de", dummy_style(), false);
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
        let model = parse_cover_letter(
            letter,
            None,
            Some("Alice"),
            "us",
            "en",
            dummy_style(),
            false,
        );
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
        let model = parse_cover_letter("x", None, None, "zz", "en", dummy_style(), false);
        assert_eq!(model.opts.page_width_mm, A4_W); // intl uses A4
    }

    /// Regression: LLM sign-off name in title-case must not be promoted to
    /// `signature_title` when `meta_name` is uppercase.
    ///
    /// Before the fix, `clean.trim() != name_text.as_str()` was a
    /// case-sensitive byte comparison.  "Saeed Kolivand" != "SAEED KOLIVAND"
    /// so the name was mistakenly stored as `signature_title` and rendered
    /// twice (bold header + plain title line).
    #[test]
    fn signoff_name_case_mismatch_not_promoted_to_signature_title() {
        let letter = "\
SAEED KOLIVAND
saeed@example.com | https://linkedin.com/in/saeedkolivand

June 10, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am excited to apply for this role and believe my background is a strong match.

Sincerely,

Saeed Kolivand
";
        let model = parse_cover_letter(
            letter,
            None,
            Some("SAEED KOLIVAND"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(
            model.signature_name, "SAEED KOLIVAND",
            "signature_name must come from meta_name"
        );
        assert!(
            model.signature_title.is_none(),
            "trailing sign-off name (title-case) must not be promoted to signature_title; \
             got {:?}",
            model.signature_title
        );
    }

    /// B.1: the applicant's own name — however the model cased it, plain or
    /// **bold** — must be skipped as a header line, never pushed into the
    /// recipient block, even when `meta_name` is stored ALL-CAPS.
    #[test]
    fn applicant_name_not_leaked_into_recipient_block() {
        for first_line in ["Saeed Kolivand", "**Saeed Kolivand**"] {
            let letter = format!(
                "\
{first_line}
saeed@example.com | https://linkedin.com/in/saeedkolivand

June 10, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am excited to apply for this role and believe my background is a strong match.

Sincerely,

Saeed Kolivand
"
            );
            let model = parse_cover_letter(
                &letter,
                None,
                Some("SAEED KOLIVAND"),
                "us",
                "en",
                dummy_style(),
                false,
            );

            assert!(
                model
                    .recipient_lines
                    .iter()
                    .any(|l| l.contains("Acme") || l.contains("Hiring Manager")),
                "recipient block should still capture the company/manager lines ({first_line:?}); \
                 got {:?}",
                model.recipient_lines
            );
            assert!(
                !model.recipient_lines.iter().any(|l| l
                    .trim()
                    .trim_end_matches([',', '.'])
                    .to_lowercase()
                    == "saeed kolivand"),
                "applicant name must not leak into the recipient block ({first_line:?}); got {:?}",
                model.recipient_lines
            );
        }
    }

    /// B.2: an ALL-CAPS stored name adopts the contact profile's mixed-case
    /// casing for BOTH letterhead and signature when the profile's `full_name`
    /// matches case-insensitively.
    #[test]
    fn all_caps_name_adopts_profile_casing() {
        let profile = ContactProfile {
            full_name: Some("Saeed Kolivand".to_string()),
            ..Default::default()
        };
        let model = parse_cover_letter(
            "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
            Some(&profile),
            Some("SAEED KOLIVAND"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(model.letterhead.name, "Saeed Kolivand");
        assert_eq!(model.signature_name, "Saeed Kolivand");
    }

    /// B.2 no-op: with no profile, an ALL-CAPS name is left untouched (never
    /// title-cased) — the stored casing is authoritative.
    #[test]
    fn all_caps_name_without_profile_stays_uppercase() {
        let model = parse_cover_letter(
            "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
            None,
            Some("SAEED KOLIVAND"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(model.letterhead.name, "SAEED KOLIVAND");
        assert_eq!(model.signature_name, "SAEED KOLIVAND");
    }

    /// B.2 no-op: a profile for a DIFFERENT person must not re-case the stored
    /// name — the casing preference is scoped to the same name only.
    #[test]
    fn all_caps_name_with_different_profile_stays_uppercase() {
        let profile = ContactProfile {
            full_name: Some("Jane Smith".to_string()),
            ..Default::default()
        };
        let model = parse_cover_letter(
            "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
            Some(&profile),
            Some("SAEED KOLIVAND"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert_eq!(model.letterhead.name, "SAEED KOLIVAND");
        assert_eq!(model.signature_name, "SAEED KOLIVAND");
    }

    // ── monogram_initials ─────────────────────────────────────────────────────
    //
    // Pure string logic behind `letter_monogram.typ`'s device (and its DOCX
    // shaded-run approximation, which calls the SAME function). Every case here
    // is one a `.typ` could not be tested on.

    #[test]
    fn monogram_initials_takes_the_first_and_last_name_tokens() {
        assert_eq!(monogram_initials("Jane Smith"), "JS");
        // First + LAST, not the first two — a `.take(2)` implementation returns
        // "JV" here, which is the wrong monogram for a multi-part surname.
        assert_eq!(monogram_initials("Jane van der Berg"), "JB");
        assert_eq!(monogram_initials("Mary Jane Watson Parker"), "MP");
    }

    #[test]
    fn monogram_initials_uppercases_and_survives_non_ascii_capitals() {
        assert_eq!(monogram_initials("àlvaro èsposito"), "ÀÈ");
        assert_eq!(monogram_initials("Àlvaro Èsposito"), "ÀÈ");
        // `char::to_uppercase` expands ß to "SS"; only the first char is taken so
        // the fixed-size device still holds exactly two glyphs.
        assert_eq!(monogram_initials("ßiggi ßmith").chars().count(), 2);
    }

    #[test]
    fn monogram_initials_handles_mononyms_and_letterless_tokens() {
        assert_eq!(monogram_initials("Prince"), "P");
        assert_eq!(monogram_initials("O'Brien"), "O");
    }

    /// A pronoun parenthetical is not a name token.
    ///
    /// The fixture is `(they/them)`, NOT `(she/her)`: with "she" the expected
    /// `JS` is also what the BROKEN implementation produces, because its `S`
    /// comes from "she" — the test passed against the defect. `they` makes the
    /// two outcomes distinguishable, `JS` (correct) vs `JT` (searched the token
    /// for its first alphanumeric instead of requiring a leading one).
    #[test]
    fn monogram_initials_drops_pronoun_parentheticals() {
        assert_eq!(monogram_initials("Jane Smith (they/them)"), "JS");
        assert_eq!(monogram_initials("Jane (they/them) Smith"), "JS");
        assert_eq!(monogram_initials("Jane (they/them)"), "J");
        // Trailing em-dash / bullet decoration must not become an initial either.
        assert_eq!(monogram_initials("Jane Smith —"), "JS");
    }

    /// Titles and qualifications are not names. A monogram for "Dr. Jane Smith"
    /// is JS; DS is the doctorate's initial standing in for the given name.
    #[test]
    fn monogram_initials_drops_titles_and_qualifications() {
        assert_eq!(monogram_initials("Dr. Jane Smith"), "JS");
        assert_eq!(monogram_initials("Prof. Dr. Jane Smith"), "JS");
        // The German honorific the critic named — "DM" was the defect.
        assert_eq!(monogram_initials("Dipl.-Ing. Max Müller"), "MM");
        assert_eq!(monogram_initials("Jane Smith Ph.D."), "JS");
    }

    /// …but a SINGLE-letter initial is part of the name and keeps counting: the
    /// title rule keys on "two or more letters before the period", so "J." is
    /// not swept up with "Dr.". Dropping it would make "J. Smith" render "S".
    #[test]
    fn monogram_initials_keeps_single_letter_initials() {
        assert_eq!(monogram_initials("J. Smith"), "JS");
        assert_eq!(monogram_initials("Jane M. Smith"), "JS");
    }

    /// A letterhead-less letter parses to an empty name; the device must then
    /// render nothing rather than an empty tinted square.
    #[test]
    fn monogram_initials_is_empty_for_a_nameless_letterhead() {
        assert_eq!(monogram_initials(""), "");
        assert_eq!(monogram_initials("   \t "), "");
        assert_eq!(monogram_initials("--- ***"), "");
    }

    /// Never more than two glyphs, whatever the name — the `.typ` device is a
    /// fixed-size square and a third initial would overflow it.
    #[test]
    fn monogram_initials_never_exceeds_two_characters() {
        for name in [
            "A B C D E F",
            "Jane Smith",
            "Prince",
            "Maria del Carmen Fernández de la Vega",
            "",
        ] {
            assert!(
                monogram_initials(name).chars().count() <= 2,
                "{name:?} produced more than two initials"
            );
        }
    }

    /// The parser must publish the initials on the letterhead — the `.typ` reads
    /// `data.letterhead.initials`, so a model that omits them silently renders an
    /// empty device.
    #[test]
    fn parsed_letterhead_carries_the_monogram_initials() {
        let model = parse_cover_letter(
            EN_LETTER,
            None,
            Some("Jane Smith"),
            "us",
            "en",
            dummy_style(),
            false,
        );
        assert_eq!(model.letterhead.initials, "JS");
    }

    /// A letterhead-less letter with no candidate name falls back to the first
    /// LINE, which is the salutation — so the device read "DM", from "Dear
    /// Hiring Manager,". Three renderer call sites pass an empty
    /// `candidate_name`, so this is reachable, not theoretical.
    ///
    /// `meta_name: Some("")` rather than `None` on purpose: that is the shape
    /// the renderer actually sends, and `parse_cover_letter` filters it to the
    /// same fallback.
    #[test]
    fn no_device_initials_when_the_name_falls_back_to_a_salutation() {
        let letterhead_less =
            "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
        for meta in [None, Some("")] {
            let model = parse_cover_letter(
                letterhead_less,
                None,
                meta,
                "us",
                "en",
                dummy_style(),
                false,
            );
            assert_eq!(
                model.letterhead.initials, "",
                "meta_name={meta:?}: the monogram device must be empty when the name is really \
                 the salutation — it rendered \"DM\" from \"Dear Hiring Manager,\""
            );
        }
    }

    /// A DATE opening is the fourth kind, and the one BOTH formats missed: the
    /// DOCX line filter excluded salutation/sign-off/subject and nothing else,
    /// so a letter starting "12 March 2025" put `12` in the device.
    #[test]
    fn no_device_initials_when_the_letter_opens_with_a_date() {
        for opening in ["12 March 2025", "2. Juni 2025", "02/06/2025", "2025-06-02"] {
            for meta in [None, Some("")] {
                let model = parse_cover_letter(
                    &format!("{opening}\n\nDear Hiring Manager,\n\nBody.\n\nSincerely,\n"),
                    None,
                    meta,
                    "us",
                    "en",
                    dummy_style(),
                    false,
                );
                assert_eq!(
                    model.letterhead.initials, "",
                    "{opening:?} (meta={meta:?}) is a date, not a name — the device must be empty"
                );
            }
        }
    }

    /// Belt to the date guard's braces: a digit can never BE an initial, so even
    /// a numeric opening the date heuristic does not recognise cannot produce
    /// one. `is_name_token` requires a leading LETTER, not merely alphanumeric.
    #[test]
    fn monogram_initials_ignores_numeric_tokens() {
        assert_eq!(monogram_initials("12 March 2025"), "M");
        assert_eq!(monogram_initials("2025"), "");
        assert_eq!(monogram_initials("42 Jane Smith 99"), "JS");
    }

    /// Same guard for the other two opening kinds the DOCX renderer already
    /// refuses to treat as a name.
    #[test]
    fn no_device_initials_for_a_signoff_or_subject_opening() {
        for opening in [
            "Mit freundlichen Grüßen,",
            "Betreff: Bewerbung als Entwickler",
        ] {
            let model = parse_cover_letter(
                &format!("{opening}\n\nBody text here.\n"),
                None,
                None,
                "de",
                "de",
                dummy_style(),
                false,
            );
            assert_eq!(
                model.letterhead.initials, "",
                "{opening:?} is not a name; the device must stay empty"
            );
        }
    }

    /// `ats` reaches `data.opts` verbatim — the whole ATS degradation story for
    /// every layout hangs off this one field being threaded, and an ATS toggle
    /// that silently fails to reach the renderer looks exactly like one that
    /// works.
    #[test]
    fn ats_flag_reaches_letter_opts() {
        let on = parse_cover_letter("x", None, None, "us", "en", dummy_style(), true);
        let off = parse_cover_letter("x", None, None, "us", "en", dummy_style(), false);
        assert!(on.opts.ats, "ats=true must reach data.opts.ats");
        assert!(!off.opts.ats, "ats=false must reach data.opts.ats");
    }
}
