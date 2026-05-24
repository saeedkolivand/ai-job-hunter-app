use regex::Regex;
use std::sync::LazyLock;

use super::types::{LineKind, ParsedDocument, ParsedLine, TextSegment};

// Lazy-initialized regexes for performance
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d{2}|20\d{2})[\s\S]{0,30}?(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d{2}|19\d{2})\b").unwrap()
});

static BULLET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([•\-–*·▪▸►✓✔○●◆◇■□▹▸]|\d+\.|[a-z]\))\s+(.+)$").unwrap()
});

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\+?\d[\d\s\-().]{7,}").unwrap()
});

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)linkedin\.com|github\.com|portfolio|website|^https?://").unwrap()
});

// Section names (multilingual)
const SECTION_NAMES: &[&str] = &[
    "professional summary", "summary", "profile", "objective", "about",
    "work experience", "experience", "employment", "employment history", "career history",
    "education", "academic background", "academic history",
    "skills", "technical skills", "core competencies", "key skills", "competencies",
    "certifications", "licenses", "credentials", "certifications & training",
    "languages", "additional languages",
    "projects", "key projects", "notable projects", "side projects",
    "achievements", "awards", "honors", "accomplishments",
    "publications", "volunteer", "volunteering", "community",
    // German
    "berufserfahrung", "arbeitserfahrung", "ausbildung", "bildung",
    "fähigkeiten", "kenntnisse", "kompetenzen", "sprachen",
    "zusammenfassung", "profil",
    // French
    "expérience professionnelle", "formation", "compétences",
];

// Company/role keywords (should NOT be treated as section headers)
const COMPANY_KEYWORDS: &[&str] = &[
    "NASA", "IBM", "AWS", "GCP", "USA", "UK", "EU", "CEO", "CTO", "VP", "SVP",
    "ENGINEER", "DEVELOPER", "MANAGER", "DIRECTOR", "LEAD", "SENIOR", "SR",
    "JUNIOR", "JR", "STAFF", "PRINCIPAL", "ARCHITECT", "ANALYST", "CONSULTANT",
    "IT", "AI", "ML", "UI", "UX", "API", "REST", "SaaS", "B2B", "B2C", "HR",
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

    // Contact: has @ or phone or pipe separators or URLs
    let pipe_count = clean.matches('|').count() + clean.matches('·').count() + clean.matches('•').count();
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

    let has_name = parsed_lines.iter().any(|l| matches!(l.kind, LineKind::Name));
    let has_contact = parsed_lines.iter().any(|l| matches!(l.kind, LineKind::Contact));
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
mod tests {
    use super::*;

    #[test]
    fn test_parse_inline_md() {
        let segments = parse_inline_md("Use **React** and **TypeScript** here");
        assert_eq!(segments.len(), 5);
        assert!(!segments[0].bold);
        assert!(segments[1].bold);
        assert_eq!(segments[1].text, "React");
    }

    #[test]
    fn test_company_name_not_section() {
        let line = parse_line("NASA ENGINEER", 1, &["Name", "NASA ENGINEER"]);
        assert!(!matches!(line.kind, LineKind::SectionHeader));
    }

    #[test]
    fn test_numbered_bullet() {
        let line = parse_line("1. First bullet point", 5, &[]);
        assert!(matches!(line.kind, LineKind::Bullet));
    }

    #[test]
    fn test_strip_md() {
        assert_eq!(strip_md("**bold**"), "bold");
        assert_eq!(strip_md("# Heading"), "Heading");
        assert_eq!(strip_md("#Heading#"), "Heading");
        assert_eq!(strip_md("## Section"), "Section");
        assert_eq!(strip_md("normal text"), "normal text");
    }

    #[test]
    fn test_section_header_detection() {
        let line = parse_line("work experience", 5, &[]);
        assert!(matches!(line.kind, LineKind::SectionHeader));
    }

    #[test]
    fn test_all_caps_section() {
        let line = parse_line("EXPERIENCE", 5, &[]);
        assert!(matches!(line.kind, LineKind::SectionHeader));
    }

    #[test]
    fn test_bullet_detection() {
        let line = parse_line("• First point", 5, &[]);
        assert!(matches!(line.kind, LineKind::Bullet));
    }

    #[test]
    fn test_job_entry_detection() {
        let line = parse_line("Software Engineer  Jan 2020 - Present", 5, &[]);
        assert!(matches!(line.kind, LineKind::JobEntry));
    }

    #[test]
    fn test_contact_detection() {
        let line = parse_line("john@example.com", 5, &[]);
        assert!(matches!(line.kind, LineKind::Contact));
    }

    #[test]
    fn test_job_title_detection() {
        let lines = vec!["Software Engineer  Jan 2020 - Present", "Senior Developer"];
        let line = parse_line("Senior Developer", 1, &lines);
        assert!(matches!(line.kind, LineKind::JobTitle));
    }

    #[test]
    fn test_multilingual_sections() {
        let line = parse_line("berufserfahrung", 5, &[]);
        assert!(matches!(line.kind, LineKind::SectionHeader));
    }

    #[test]
    fn test_parse_resume() {
        let text = "John Doe\njohn@example.com\n\nExperience\nSoftware Engineer  Jan 2020 - Present";
        let doc = parse_resume(text);
        assert!(doc.has_name);
        assert!(doc.has_contact);
        assert!(doc.section_count > 0);
    }
}
