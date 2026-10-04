use regex::Regex;
use std::sync::LazyLock;

use super::types::{LineKind, ParsedDocument, ParsedLine};

// Split by responsibility (issue #1280 batch 5b): text normalization,
// inline-Markdown handling, line-shape heuristics, and section-heading
// recognition each get their own module; this file keeps the per-line
// classifier (`parse_line`) that ties them together plus the public
// `parse_resume` entry point. Re-exports below keep every external caller's
// `crate::export::parser::X` path unchanged.
mod headings;
mod markdown;
mod normalize;
mod shapes;

pub(crate) use headings::{is_all_caps_section_heading, is_known_section_name, strip_atx_heading};
pub use markdown::{parse_inline_md, strip_md};
pub(crate) use normalize::is_word_char;
pub use normalize::{is_private_use, normalize_unicode, sanitize_markdown, typography};
pub(crate) use shapes::{
    is_contact_shaped, is_first_line_contact_shaped, is_project_stack_shaped,
    is_project_title_shaped,
};

use headings::{despace_letterspaced, is_thematic_break};
use shapes::{
    is_contactish_segment, is_entry_title_shaped, separator_count, BULLET_RE, DATE_RE, SOLO_DATE_RE,
};

/// Parse a single line
fn parse_line(raw: &str, idx: usize, all_lines: &[&str]) -> ParsedLine {
    let trimmed = raw.trim();
    let mut clean = strip_md(trimmed);
    // A letter-spaced heading is rewritten to its collapsed form ONLY when that
    // form is a heading we know. Gating on the result means an ordinary line can
    // never be rewritten by accident, and every downstream consumer
    // (`SectionId::from_header`, `documents::evidence`, the renderer) sees the
    // same readable text.
    if let Some(despaced) = despace_letterspaced(&clean) {
        if is_known_section_name(&despaced) {
            clean = despaced;
        }
    }
    let clean = clean;
    let segments = parse_inline_md(trimmed);

    // Blank line
    if clean.is_empty() {
        return ParsedLine {
            kind: LineKind::Blank,
            raw: String::new(),
            text: String::new(),
            segments: Vec::new(),
            right_text: None,
        };
    }

    // Markdown thematic break (`---`, `***`, `___`): a visual separator, never
    // content. Dropped as Blank so it doesn't render as a stray "---" paragraph
    // on top of the template's own section rule. Checked on `trimmed` (not
    // `clean`, which collapses `***` → `*` via the `**` bold strip).
    if is_thematic_break(trimmed) {
        return ParsedLine {
            kind: LineKind::Blank,
            raw: String::new(),
            text: String::new(),
            segments: Vec::new(),
            right_text: None,
        };
    }

    // Explicit Markdown ATX heading (`# `/`## `/`### ` … up to `######`). A
    // user-authored custom heading like `## Side Projects` is promoted to a
    // section heading regardless of whether it matches a known section name or is
    // ALL-CAPS — guaranteeing editor-created headings render via
    // `SectionId::from_header` → `Custom(name)`. Runs before the idx==0
    // name/contact block and the all-caps / job-entry / contact branches, but
    // after the Blank and thematic-break checks: a `---`/`***` break is still
    // dropped as Blank, and a bare `# ` whose heading text is empty falls through
    // to Blank rather than emitting an empty heading.
    if let Some(heading_body) = strip_atx_heading(trimmed) {
        let text = strip_md(heading_body);
        if !text.is_empty() {
            return ParsedLine {
                kind: LineKind::SectionHeader,
                raw: trimmed.to_string(),
                text,
                // Parse inline marks from the marker-stripped body so a bold run
                // inside the heading (`## **Lead**`) still tokenizes.
                segments: parse_inline_md(heading_body),
                right_text: None,
            };
        }
    }

    // First line WITH CONTENT is name ONLY if it doesn't look like a section
    // header or contact — NOT literally raw line 0. PDF extraction routinely
    // emits a leading blank line before the real header text, so gating on
    // `idx == 0` misses the name entirely (it lands at idx 1+), falls through
    // to the ALL-CAPS section-header branch below, and the header renders
    // twice. `idx == 0` is kept as its own arm so this can never regress the
    // original rule, whatever `all_lines` holds. Past that, the preceding
    // lines must all be blank AND `idx` must be a real index into `all_lines`
    // — an out-of-bounds range yields `None` rather than a vacuously-true
    // empty slice, which is what unit tests calling `parse_line(x, 5, &[])`
    // rely on to mean "not the first line". `.all()` short-circuits at the
    // first non-blank line, so this stays effectively constant-time (blank
    // lines return early above, so only the first content line reaches here).
    if idx == 0
        || all_lines
            .get(..idx)
            .is_some_and(|head| head.iter().all(|l| l.trim().is_empty()))
    {
        if is_known_section_name(&clean) {
            return ParsedLine {
                kind: LineKind::SectionHeader,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments,
                right_text: None,
            };
        }
        if is_first_line_contact_shaped(&clean) {
            return ParsedLine {
                kind: LineKind::Contact,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments,
                right_text: None,
            };
        }
        return ParsedLine {
            kind: LineKind::Name,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Bullet detection. Matched against `trimmed` (not `clean`) so a
    // `**bold**` marker or the bullet's own markdown survives into
    // `raw`/`segments` — matching on the already-markdown-stripped `clean`
    // here was the root cause of `**bold**` never rendering in any bullet, in
    // any template, PDF or DOCX: the marker was gone before `raw` was ever
    // captured.
    if let Some(caps) = BULLET_RE.captures(trimmed) {
        if let Some(bullet_text) = caps.get(2) {
            let text = bullet_text.as_str();
            return ParsedLine {
                kind: LineKind::Bullet,
                raw: text.to_string(),
                text: strip_md(text),
                segments: parse_inline_md(text),
                right_text: None,
            };
        }
    }

    // Tab-indented bullet
    if raw.starts_with('\t') && clean.len() > 5 && !is_known_section_name(&clean) {
        return ParsedLine {
            kind: LineKind::Bullet,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Section header: known name (including a two-section "X & Y" ampersand
    // join where both halves are known — see `is_known_section_name`)
    if is_known_section_name(&clean) {
        return ParsedLine {
            kind: LineKind::SectionHeader,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // All-caps detection (but exclude company names and roles)
    if is_all_caps_section_heading(&clean) {
        return ParsedLine {
            kind: LineKind::SectionHeader,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Job entry: 2+ spaces gap before a date range. Located in `trimmed` (not
    // `clean`) so `raw`/`segments` carry the title's own markdown through to
    // the model — matching on the already-stripped `clean` here silently
    // erased a bold job title before `raw` was ever captured. `raw` holds
    // just the title (mirrors the other two `JobEntry` forms below, whose
    // whole line IS the title); the date stays in `right_text`.
    if let Some(idx) = trimmed.find("  ") {
        let left_raw = &trimmed[..idx];
        let right_raw = trimmed[idx..].trim_start();
        let right_clean = strip_md(right_raw);

        if DATE_RE.is_match(&right_clean) {
            let left_clean = strip_md(left_raw);
            let word_count = left_clean.split_whitespace().count();
            if word_count >= 2 || left_clean.len() > 10 {
                return ParsedLine {
                    kind: LineKind::JobEntry,
                    raw: left_raw.to_string(),
                    text: left_clean,
                    segments: parse_inline_md(left_raw),
                    right_text: Some(right_clean),
                };
            }
        }
    }

    // Job entry: trailing parenthesized date — "Role Title, Company Name (January 2021 – March 2023)"
    // The whole line (role + company + period) becomes the bold entry title.
    {
        static PAREN_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?i)^(.+?)\s*\(\s*(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d{2}|20\d{2})[\s\S]{0,30}?(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d{2}|19\d{2})\s*\)\s*$").unwrap()
        });
        if PAREN_DATE_RE.is_match(&clean) && !clean.contains('@') {
            return ParsedLine {
                kind: LineKind::JobEntry,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments: parse_inline_md(trimmed),
                right_text: None,
            };
        }
    }

    // Job entry: pipe/middot-separated with a date segment and no email address —
    // "Role | Company | 2020 – Present" or "Role · Company · Jan 2021 – Mar 2023".
    // Excludes contact lines: an email, or a non-date phone/URL segment, keeps Contact.
    let pipe_count = separator_count(&clean);
    if pipe_count >= 1 && !clean.contains('@') {
        let seg_is_date = |s: &str| DATE_RE.is_match(s) || SOLO_DATE_RE.is_match(s);
        // A non-date phone/URL segment marks this as a CONTACT line, not an entry
        // (e.g. "Berlin | +49 30 1234567 | 2021" or "City | linkedin.com/in/x | 2021").
        // The `@`-only guard was insufficient: real contacts carry a phone/URL, no email.
        let has_contact_segment = clean.split(['|', '·', '•']).any(|seg| {
            let s = seg.trim();
            is_contactish_segment(s) && !seg_is_date(s)
        });
        let has_range = clean
            .split(['|', '·', '•'])
            .any(|seg| DATE_RE.is_match(seg.trim()));
        let has_solo = clean
            .split(['|', '·', '•'])
            .any(|seg| SOLO_DATE_RE.is_match(seg.trim()));
        // A date RANGE is entry-like even with a single separator. A bare single
        // year is ambiguous with skill/cert lines ("AWS Certified • 2023"), so it
        // only counts as an entry when there are ≥2 separators ("Name | Type | 2021").
        if !has_contact_segment && (has_range || (has_solo && pipe_count >= 2)) {
            return ParsedLine {
                kind: LineKind::JobEntry,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments: parse_inline_md(trimmed),
                right_text: None,
            };
        }
    }

    // Job entry: a title-shaped line immediately followed, on its OWN line, by
    // a bare leading date range — "Title · Company" \n "Mon YYYY – Mon
    // YYYY[, Location]" — the shape none of the same-line patterns above
    // cover (see `is_entry_title_shaped`'s doc comment). The date is read off
    // the next line and carried as `right_text`; the paired branch below
    // (checked on the date line itself) consumes it so it never ALSO renders
    // as a second, unrelated body line.
    if is_entry_title_shaped(&clean) {
        if let Some(next_line) = all_lines.get(idx + 1) {
            if let Some(m) = DATE_RE.find(next_line.trim()) {
                if m.start() == 0 {
                    return ParsedLine {
                        kind: LineKind::JobEntry,
                        raw: trimmed.to_string(),
                        text: clean.clone(),
                        segments: parse_inline_md(trimmed),
                        right_text: Some(m.as_str().to_string()),
                    };
                }
            }
        }
    }

    // The other half of the pair above: THIS line is a leading date range and
    // the PREVIOUS line was just claimed as that entry's title. Drop the
    // matched date; whatever remains (a comma- or dash-led location /
    // description) becomes the entry's subtitle. Nothing remaining collapses
    // to Blank — the date has already been attached to the entry above.
    if idx > 0 {
        if let Some(m) = DATE_RE.find(trimmed) {
            if m.start() == 0 {
                let prev_clean = strip_md(all_lines.get(idx - 1).unwrap_or(&"").trim());
                if is_entry_title_shaped(&prev_clean) {
                    let remainder = trimmed[m.end()..]
                        .trim_start_matches([',', '-', '\u{2013}', '\u{2014}', ' '])
                        .trim();
                    if remainder.is_empty() {
                        return ParsedLine {
                            kind: LineKind::Blank,
                            raw: String::new(),
                            text: String::new(),
                            segments: Vec::new(),
                            right_text: None,
                        };
                    }
                    return ParsedLine {
                        kind: LineKind::JobTitle,
                        raw: remainder.to_string(),
                        text: strip_md(remainder),
                        segments: parse_inline_md(remainder),
                        right_text: None,
                    };
                }
            }
        }
    }

    // Contact: has @ or phone or pipe separators or URLs
    // (pipe_count was computed above for the pipe-date job-entry check)
    if is_contact_shaped(&clean) {
        return ParsedLine {
            kind: LineKind::Contact,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Job title: short line immediately after a job entry
    if idx > 0 && clean.len() < 100 {
        let prev = all_lines.get(idx - 1).unwrap_or(&"");
        let prev_clean = strip_md(prev.trim());

        if prev_clean.contains("  ") && DATE_RE.is_match(&prev_clean) {
            return ParsedLine {
                kind: LineKind::JobTitle,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments,
                right_text: None,
            };
        }
    }

    // Default: text
    ParsedLine {
        kind: LineKind::Text,
        raw: trimmed.to_string(),
        text: clean,
        segments,
        right_text: None,
    }
}

/// Parse resume text into structured document
pub fn parse_resume(text: &str) -> ParsedDocument {
    let lines: Vec<&str> = text.lines().collect();
    let parsed_lines: Vec<ParsedLine> = lines
        .iter()
        .enumerate()
        .map(|(idx, line)| parse_line(line, idx, &lines))
        .collect();

    let has_name = parsed_lines
        .iter()
        .any(|l| matches!(l.kind, LineKind::Name));
    let has_contact = parsed_lines
        .iter()
        .any(|l| matches!(l.kind, LineKind::Contact));
    let section_count = parsed_lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::SectionHeader))
        .count();

    ParsedDocument {
        lines: parsed_lines,
        has_name,
        has_contact,
        section_count,
    }
}

#[cfg(test)]
mod tests;
