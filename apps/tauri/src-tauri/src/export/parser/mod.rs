use regex::Regex;
use std::sync::LazyLock;

use super::types::{LineKind, ParsedDocument, ParsedLine, TextSegment};

/// Replace Unicode characters that embedded PDF fonts cannot render with safe
/// ASCII or Latin-1 equivalents. Applied before any text hits the PDF renderer.
pub fn normalize_unicode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let replacement: &str = match ch {
            // Dashes / hyphens. En-dash (U+2013) and em-dash (U+2014) are PRESERVED
            // (the bundled fonts contain both glyphs — asserted by a unit test); the
            // later `typography` pass normalizes their spacing. Collapsing them to a
            // bare hyphen used to mangle sentence-break dashes into "word- word".
            '\u{2010}' | '\u{2011}' | '\u{2012}' => "-", // hyphen / non-breaking hyphen / figure dash
            '\u{2015}' => "\u{2014}",                    // horizontal bar → em-dash
            '\u{2212}' => "-",                           // minus sign
            // Quotes
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => "\"", // double quotes
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => "'", // single quotes / apostrophes
            '\u{2032}' => "'",                                        // prime
            '\u{2033}' => "\"",                                       // double prime
            // Spaces / invisible chars
            '\u{00A0}' | '\u{202F}' | '\u{2007}' | '\u{2008}' => " ", // non-breaking / narrow no-break / figure / punctuation space
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => "",  // zero-width spaces / BOM
            '\u{00AD}' => "-",                                        // soft hyphen
            // Ellipsis
            '\u{2026}' => "...",
            // Bullets / symbols
            '\u{2022}' | '\u{2023}' | '\u{2043}' | '\u{204C}' | '\u{204D}' => "-", // bullet variants
            '\u{25E6}' | '\u{2219}' | '\u{22C5}' => "-", // white bullet / bullet operator / dot operator
            // Arrows
            '\u{2192}' => "->",
            '\u{2190}' => "<-",
            '\u{2194}' => "<->",
            '\u{21D2}' => "=>",
            '\u{2191}' => "^",
            '\u{2193}' => "v",
            // Trademark / legal
            '\u{2122}' => "(TM)",
            '\u{00AE}' => "(R)",
            '\u{00A9}' => "(c)",
            // Multiplication / fractions
            '\u{00D7}' => "x",
            '\u{00F7}' => "/",
            '\u{00BD}' => "1/2",
            '\u{00BC}' => "1/4",
            '\u{00BE}' => "3/4",
            // Superscripts
            '\u{00B2}' => "2",
            '\u{00B3}' => "3",
            '\u{00B9}' => "1",
            // Other
            '\u{2116}' => "No.",
            '\u{2020}' | '\u{2021}' => "", // daggers — drop (never emit a stray asterisk)
            '\u{00B7}' => ".",             // middle dot
            // Private Use Area + icon-font glyphs + replacement char: these render
            // as boxes/garbage (or nothing) in the bundled fonts — drop them.
            c if is_private_use(c) || c == '\u{FFFD}' => "",
            // C0/C1 control characters other than the whitespace we keep.
            c if c.is_control() && c != '\n' && c != '\r' && c != '\t' => "",
            _ => {
                out.push(ch);
                continue;
            }
        };
        out.push_str(replacement);
    }
    out
}

/// Strip stray Markdown emphasis the model occasionally leaks (`*React`, `AWS*`,
/// `AWS*-Services`) WITHOUT touching valid `**bold**` runs (the renderer turns those
/// into real bold) or in-word punctuation like `snake_case`. Runs after
/// [`normalize_unicode`], before any Markdown parsing.
pub fn sanitize_markdown(text: &str) -> String {
    // Protect valid bold pairs, drop every remaining lone '*' and stray backtick,
    // then restore the bold markers.
    const BOLD: &str = "\u{0}B\u{0}";
    text.replace("**", BOLD)
        .chars()
        .filter(|&c| c != '*' && c != '`')
        .collect::<String>()
        .replace(BOLD, "**")
}

/// Typography pass for dash usage. With en/em-dashes preserved by
/// [`normalize_unicode`], normalize clause-level dash spacing to a spaced en-dash
/// (" – ") and rewrite the residual ASCII "word- word" sentence-break pattern the
/// same way — but never a German suspended hyphen (`Backend- und …`) or a tight
/// compound / range (`2020–2023`, `state-of-the-art`).
pub fn typography(text: &str) -> String {
    let out = HYPHEN_BREAK_RE.replace_all(text, |c: &regex::Captures| {
        let prev = &c[1];
        let next = &c[2];
        if SUSPENDED_HYPHEN_WORDS.contains(&next.to_lowercase().as_str()) {
            format!("{prev}- {next}")
        } else {
            format!("{prev} \u{2013} {next}")
        }
    });
    DASH_CLAUSE_SPACING_RE
        .replace_all(&out, " \u{2013} ")
        .into_owned()
}

/// A complete word, an ASCII hyphen, a space, then the next word — a sentence-break
/// hyphen the model sometimes emits instead of a dash. (`e-mail`, `state-of-the-art`
/// have no space after the hyphen and never match.)
static HYPHEN_BREAK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\p{L})- (\p{L}[\p{L}.]*)").unwrap());

/// An en/em-dash used between clauses (a space on at least one side) → cleanly spaced
/// en-dash. A tight range like `2020–2023` has no surrounding space and is left alone.
static DASH_CLAUSE_SPACING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[\u{2013}\u{2014}]\s+|\s+[\u{2013}\u{2014}]\s*").unwrap());

/// German suspended-hyphen continuations: `Backend- und Frontend-…` is correct German
/// and must keep its hyphen rather than become a dash.
const SUSPENDED_HYPHEN_WORDS: &[&str] = &[
    "und",
    "oder",
    "bzw",
    "sowie",
    "als",
    "wie",
    "bis",
    "beziehungsweise",
    "respektive",
];

/// Unicode Private Use Area code points (BMP + planes 15/16). Icon fonts
/// (Font Awesome, etc.) map glyphs here, so extracted/pasted text often contains
/// PUA code points that are meaningless without the original font.
pub fn is_private_use(c: char) -> bool {
    matches!(
        c as u32,
        0xE000..=0xF8FF | 0xF_0000..=0xF_FFFD | 0x10_0000..=0x10_FFFD
    )
}

// Lazy-initialized regexes for performance
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d{2}|20\d{2})[\s\S]{0,30}?(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d{2}|19\d{2})\b").unwrap()
});

// A pipe/middot segment that IS a standalone single date — a bare year or
// "Month YYYY" (e.g. "2021", "Jan 2021"). Anchored so it matches only when the
// whole segment is a date. Lets single-year entries (common for PROJECTS and
// education) be recognized as entries, not only date ranges.
static SOLO_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\.?\s+)?(?:19|20)\d{2}\s*$").unwrap()
});

static BULLET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([•\-–*·▪▸►✓✔○●◆◇■□▹▸]|\d+\.|[a-z]\))\s+(.+)$").unwrap());

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+?\d[\d\s\-().]{7,}").unwrap());

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)linkedin\.com|github\.com|portfolio|website|^https?://").unwrap()
});

// Section names (multilingual)
const SECTION_NAMES: &[&str] = &[
    "professional summary",
    "summary",
    "profile",
    "objective",
    "about",
    "work experience",
    "experience",
    "employment",
    "employment history",
    "career history",
    "education",
    "academic background",
    "academic history",
    "skills",
    "technical skills",
    "core competencies",
    "key skills",
    "competencies",
    "certifications",
    "licenses",
    "credentials",
    "certifications & training",
    "languages",
    "additional languages",
    "projects",
    "key projects",
    "notable projects",
    "side projects",
    "achievements",
    "awards",
    "honors",
    "accomplishments",
    "publications",
    "volunteer",
    "volunteering",
    "community",
    // German
    "berufserfahrung",
    "arbeitserfahrung",
    "ausbildung",
    "bildung",
    "fähigkeiten",
    "kenntnisse",
    "kompetenzen",
    "sprachen",
    "zusammenfassung",
    "profil",
    // French
    "expérience professionnelle",
    "formation",
    "compétences",
];

// Company/role keywords (should NOT be treated as section headers)
const COMPANY_KEYWORDS: &[&str] = &[
    "NASA",
    "IBM",
    "AWS",
    "GCP",
    "USA",
    "UK",
    "EU",
    "CEO",
    "CTO",
    "VP",
    "SVP",
    "ENGINEER",
    "DEVELOPER",
    "MANAGER",
    "DIRECTOR",
    "LEAD",
    "SENIOR",
    "SR",
    "JUNIOR",
    "JR",
    "STAFF",
    "PRINCIPAL",
    "ARCHITECT",
    "ANALYST",
    "CONSULTANT",
    "IT",
    "AI",
    "ML",
    "UI",
    "UX",
    "API",
    "REST",
    "SaaS",
    "B2B",
    "B2C",
    "HR",
];

/// Parse **bold** markers into text segments
pub fn parse_inline_md(line: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_bold = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'*') {
            // Found ** marker
            chars.next(); // consume second *

            // Save current segment if any
            if !current.is_empty() {
                segments.push(TextSegment {
                    text: current.clone(),
                    bold: in_bold,
                });
                current.clear();
            }

            // Toggle bold state
            in_bold = !in_bold;
        } else {
            current.push(ch);
        }
    }

    // Save final segment
    if !current.is_empty() {
        segments.push(TextSegment {
            text: current,
            bold: in_bold,
        });
    }

    if segments.is_empty() {
        segments.push(TextSegment {
            text: line.to_string(),
            bold: false,
        });
    }

    segments
}

/// Strip **bold** markers and leading/trailing # heading markers from text
pub fn strip_md(text: &str) -> String {
    let no_bold = text.replace("**", "");
    let s = no_bold.trim_start_matches('#').trim_start();
    s.trim_end_matches('#').trim_end().to_string()
}

/// Check if all-caps text is likely a company/role name
fn is_likely_company_or_role(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    words.iter().any(|word| COMPANY_KEYWORDS.contains(word))
}

/// A Markdown thematic break: 3+ identical `-`, `*`, or `_` markers (optionally
/// separated by spaces) and nothing else — e.g. `---`, `***`, `___`, `- - -`.
/// The model emits these as section separators, but every template already draws
/// its own section rules, so a literal break renders as stray "---" text AND
/// doubles the divider. Recognized here so it can be dropped as a blank line.
fn is_thematic_break(line: &str) -> bool {
    let marks: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    if marks.len() < 3 {
        return false;
    }
    let first = marks.chars().next().unwrap();
    matches!(first, '-' | '*' | '_') && marks.chars().all(|c| c == first)
}

/// If `line` begins with a Markdown ATX heading marker — a run of 1–6 `#` followed
/// by a space (`# `, `## `, … `###### `) — return the content after that marker
/// (with the leading `#`/space prefix removed but inline `**bold**` preserved).
/// A `#hashtag` with no trailing space is NOT a heading and yields `None`. This is
/// what lets a user-authored custom heading (`## Side Projects`) always classify
/// as a section heading, independent of the known-name / ALL-CAPS heuristics.
fn strip_atx_heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some(line[hashes..].trim_start())
    } else {
        None
    }
}

/// Parse a single line
fn parse_line(raw: &str, idx: usize, all_lines: &[&str]) -> ParsedLine {
    let trimmed = raw.trim();
    let clean = strip_md(trimmed);
    let segments = parse_inline_md(trimmed);
    let lower = clean.to_lowercase();

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

    // First line is name ONLY if it doesn't look like a section header or contact
    if idx == 0 {
        if SECTION_NAMES.contains(&lower.as_str()) {
            return ParsedLine {
                kind: LineKind::SectionHeader,
                raw: trimmed.to_string(),
                text: clean.clone(),
                segments,
                right_text: None,
            };
        }
        if clean.contains('@') || PHONE_RE.is_match(&clean) {
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

    // Bullet detection
    if let Some(caps) = BULLET_RE.captures(&clean) {
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
    if raw.starts_with('\t') && clean.len() > 5 && !SECTION_NAMES.contains(&lower.as_str()) {
        return ParsedLine {
            kind: LineKind::Bullet,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Section header: known name
    if SECTION_NAMES.contains(&lower.as_str()) {
        return ParsedLine {
            kind: LineKind::SectionHeader,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // All-caps detection (but exclude company names and roles)
    if clean == clean.to_uppercase()
        && clean.len() >= 4
        && clean.len() <= 60
        && clean.chars().filter(|c| c.is_alphabetic()).count() >= 2
        && !clean.chars().any(|c| c.is_ascii_digit() && clean.matches(c).count() == 4) // No years
        && !is_likely_company_or_role(&clean)
        && !DATE_RE.is_match(&clean)
        && !clean.contains('@')
    {
        return ParsedLine {
            kind: LineKind::SectionHeader,
            raw: trimmed.to_string(),
            text: clean.clone(),
            segments,
            right_text: None,
        };
    }

    // Job entry: 2+ spaces gap before a date range
    if let Some(idx) = clean.find("  ") {
        let left = &clean[..idx];
        let right = &clean[idx..].trim_start();

        if DATE_RE.is_match(right) {
            let word_count = left.split_whitespace().count();
            if word_count >= 2 || left.len() > 10 {
                return ParsedLine {
                    kind: LineKind::JobEntry,
                    raw: trimmed.to_string(),
                    text: left.to_string(),
                    segments: parse_inline_md(left),
                    right_text: Some(right.to_string()),
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
    let pipe_count =
        clean.matches('|').count() + clean.matches('·').count() + clean.matches('•').count();
    if pipe_count >= 1 && !clean.contains('@') {
        let seg_is_date = |s: &str| DATE_RE.is_match(s) || SOLO_DATE_RE.is_match(s);
        // A non-date phone/URL segment marks this as a CONTACT line, not an entry
        // (e.g. "Berlin | +49 30 1234567 | 2021" or "City | linkedin.com/in/x | 2021").
        // The `@`-only guard was insufficient: real contacts carry a phone/URL, no email.
        let has_contact_segment = clean.split(['|', '·', '•']).any(|seg| {
            let s = seg.trim();
            (PHONE_RE.is_match(s) || URL_RE.is_match(s)) && !seg_is_date(s)
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

    // Contact: has @ or phone or pipe separators or URLs
    // (pipe_count was computed above for the pipe-date job-entry check)
    if clean.contains('@')
        || PHONE_RE.is_match(&clean)
        || pipe_count >= 2
        || URL_RE.is_match(&clean)
    {
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
mod test;
