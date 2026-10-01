//! Cover-letter parser for the Typst rendering engine.
//!
//! [`parse_cover_letter`] splits a finished letter text into letterhead /
//! date / recipient / subject / salutation / body / signoff / signature,
//! reusing [`crate::locale::letter`] detection helpers so the engine
//! recognises all supported markets and salutations.
//!
//! **Injection safety**: the [`model`] is serialised to JSON and injected
//! via the virtual `data.json` — no user content is ever concatenated into
//! Typst markup.
//!
//! Offline hard-wall: this file contains NO typst / typst_pdf imports.
//! All Typst types remain inside `engine.rs` and `render.rs`.

// `pub(super)`: `LetterStyle` (declared inside) is also needed by the sibling
// `letterhead` module's own tests (`dummy_style()`), matching its reach before this split —
// it was a direct `pub(super)` item of this file, visible anywhere under `typst_engine`.
pub(super) mod model;

use super::letterhead::{
    is_letterhead_name, letterhead_initials, looks_like_date, resolve_letterhead_candidate,
};
use crate::contact_profile::ContactProfile;
use crate::locale::letter::conventions;
use crate::model::rich::tokenize_rich;

use model::{page_dims, LetterHead, LetterModel, LetterOpts, LetterRun, LetterStyle};

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

/// Parse a finished cover-letter text into a structured [`LetterModel`].
///
/// Parsing rules:
/// - Skips leading header lines (name + contact line echoed from the text).
/// - Recognises salutations/sign-offs via `locale::letter` helpers.
/// - Pre-salutation lines: subject (`is_subject_line`), date
///   (`looks_like_date`), or recipient.
/// - Body paragraphs are parsed as rich-text runs via `tokenize_rich` so
///   **bold** phrases survive.
/// - Post-signoff: first non-blank non-name, non-placeholder line is
///   signature_title (an unfilled template slot like "Ihr Name" is dropped,
///   never promoted — see [`crate::locale::is_template_placeholder`]).
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
    // Name: prefer generation metadata, then the contact profile's full name,
    // then the first non-blank text line.
    let raw_lines: Vec<&str> = text.lines().collect();

    // The missing middle rung: the renderer always sends the full
    // `ContactProfile`, but until now it was consulted only for CASING
    // (`prefer_profile_casing` below) — a blank `meta_name` fell straight
    // through to the first-line fallback even when a real name sat right
    // there on `contact.full_name`. That fallback is not just a body-text
    // quality issue: `document_meta_preamble("data.letterhead.name", …)`
    // (`engine.rs`) feeds this same value into the PDF's `author` metadata
    // field, so a blank `meta_name` with a body-only (letterhead-less) letter
    // produced an empty PDF author too.
    let contact_full_name = contact
        .and_then(|p| p.full_name.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // B.2: an ALL-CAPS stored name adopts the contact profile's mixed-case
    // casing (when it matches case-insensitively) so the letterhead + signature
    // don't render shouted; no-op with no profile or a different-name profile.
    //
    // `resolve_letterhead_candidate` is the shared "prefer meta_name unless
    // blank" decision — both DOCX line-scanners make the identical call, so
    // an empty-string `Some("")` `meta_name` (the shape three renderer call
    // sites actually send) degrades to the fallback the same way in every
    // format.
    let name_text: String = prefer_profile_casing(
        resolve_letterhead_candidate(meta_name, || {
            contact_full_name.unwrap_or_else(|| {
                raw_lines
                    .iter()
                    .map(|l| l.trim())
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
            })
        })
        .to_string(),
        contact,
    );

    // Suppress the letterhead NAME entirely when it fails the same is-a-name
    // test the monogram device uses. With no `meta_name` supplied, `name_text`
    // above just fell back to the letter's first non-blank line, and that line
    // can be a date ("12 March 2025") or a salutation ("Dear Hiring Manager,")
    // rather than a name — every `.typ` layout reads `data.letterhead.name`
    // (and `signature_name`, which is the same value) verbatim, so an
    // unguarded fallback rendered the wrong text as the person's name in all
    // six layouts, not just Monogram's already-guarded device.
    //
    // This also fixes a second-order bug the fabricated name caused: the
    // header-dedupe skip below (`clean == name_lower`) matched this exact
    // line and ate it as a duplicate header echo, silently dropping the
    // date/salutation from the rest of the parse. An empty `name_text` no
    // longer matches anything there, so the line falls through to the
    // ordinary date/salutation/subject/recipient classification instead —
    // the letter has no letterhead name, but the body still carries the line.
    let name_text = if is_letterhead_name(&name_text) {
        name_text
    } else {
        String::new()
    };

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
    //
    // Trailing-punctuation-trimmed to match the `clean` side of every
    // comparison against this value (`clean.trim().trim_end_matches([',',
    // '.'])…`) below — a name that legitimately ends in a period never
    // matched its own header/signature echo before this, because `clean` had
    // its trailing `.`/`,` stripped and this side did not.
    let name_lower = name_text.trim().trim_end_matches([',', '.']).to_lowercase();

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
            // `signature_title` and rendered a second time. Reuses `name_lower`
            // (already trailing-punctuation-trimmed above) rather than
            // recomputing `name_text.trim().to_lowercase()` here — that
            // untrimmed recomputation was the same asymmetry bug as the
            // header-echo skip above: a name ending in "." never matched.
            let is_candidate_name =
                clean.trim().trim_end_matches([',', '.']).to_lowercase() == name_lower;
            // An unfilled template slot ("Ihr Name", "[Job Title]") is not a
            // real title — never promote it (ADR-034 Consequence #2). Test the
            // RAW line too: `clean` has been through `strip_md_links`, which
            // unwraps any whole `[...]` span, so the bracket branch alone dies.
            let is_placeholder = crate::locale::is_template_placeholder(trimmed)
                || crate::locale::is_template_placeholder(&clean);
            if signature_title.is_none()
                && !clean.is_empty()
                && !is_candidate_name
                && !is_placeholder
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
    // Shared with `render::style_from_template` (JsonStyle) rather than a
    // byte-identical private copy — one hex formatter, one font-family map.
    use super::render::{font_family_to_typst, rgb_to_hex};

    LetterStyle {
        c_accent: rgb_to_hex(t.accent_color.0, t.accent_color.1, t.accent_color.2),
        c_body: rgb_to_hex(t.body_color.0, t.body_color.1, t.body_color.2),
        c_name: rgb_to_hex(t.name_color.0, t.name_color.1, t.name_color.2),
        c_date: rgb_to_hex(t.date_color.0, t.date_color.1, t.date_color.2),
        c_rule: rgb_to_hex(t.rule_color.0, t.rule_color.1, t.rule_color.2),
        font_name: font_family_to_typst(t.fonts.name_family).to_string(),
        font_body: font_family_to_typst(t.fonts.body_family).to_string(),
        name_pt: t.name_pt,
        body_pt: t.body_pt,
    }
}

#[cfg(test)]
mod tests;
