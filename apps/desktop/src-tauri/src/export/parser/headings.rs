//! Section-heading recognition: the known multilingual section-name list, the
//! generic ALL-CAPS heading rule, letter-spaced-heading collapsing, Markdown
//! ATX headings, and thematic breaks.

use super::shapes::DATE_RE;

// Section names (multilingual)
pub(super) const SECTION_NAMES: &[&str] = &[
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
    "core skills",
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
    "selected projects",
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
    "projekte",
    "zertifikate",
    "auszeichnungen",
    "publikationen",
    // French
    "expérience professionnelle",
    "formation",
    "compétences",
    "projets",
    "langues",
    "distinctions",
    // Spanish (shares "perfil" with Portuguese below)
    "perfil",
    "experiencia profesional",
    "formación",
    "habilidades",
    "proyectos",
    "certificaciones",
    "idiomas",
    "premios",
    "publicaciones",
    // Italian
    "profilo",
    "esperienza professionale",
    "formazione",
    "competenze",
    "progetti",
    "certificazioni",
    "lingue",
    "riconoscimenti",
    "pubblicazioni",
    // Dutch
    "profiel",
    "werkervaring",
    "opleiding",
    "vaardigheden",
    "projecten",
    "certificaten",
    "talen",
    "onderscheidingen",
    "publicaties",
    // Portuguese
    "experiência profissional",
    "formação",
    "competências",
    "projetos",
    "certificações",
    // "idiomas" (Languages) is shared with Spanish above — the producer
    // (`pipeline::resume::prompt_blocks::resume_conventions`) emits the same
    // word for both locales, so one entry covers both.
    // pt-PT "prémios" is what the producer actually emits (see
    // prompt_blocks.rs); pt-BR "prêmios" is added too — nothing in the
    // producer's header table discriminates the two spellings, so the
    // recogniser accepts both even though only one is ever generated.
    "prémios",
    "prêmios",
    "publicações",
];

/// Whether `text`, trimmed and lowercased, exactly names one of the parser's
/// own known section headings — the same exact-match test [`parse_line`] uses
/// to promote a line to [`LineKind::SectionHeader`] via [`SECTION_NAMES`] —
/// OR is a two-section AMPERSAND JOIN ("Ausbildung & Sprachen") where BOTH
/// halves are themselves known names.
///
/// The résumé prompt's own "never combine two sections under a joined
/// heading" clause (`pipeline::resume::prompts::draft_system`) means a fresh
/// generation should not produce this shape, but it still shows up on
/// already-generated documents and on an occasional non-compliant
/// generation — that is exactly how "Ausbildung & Sprachen" was reported.
/// This does NOT split the line into two sections: the bullets underneath a
/// merged heading have no reliable per-line split point (there is nothing on
/// a bullet that says which half of the join it belongs to), so splitting
/// would risk silently misattributing content, a worse outcome than one
/// heading covering both. Recognising the join only keeps it OFF the
/// body-text path: a single, non-ideal-but-visible heading beats a merged
/// heading rendering as an unstyled paragraph, both for our own rendering and
/// for a real ATS reading the exported PDF/DOCX, which buckets by heading
/// text and gets zero heading signal from a plain paragraph line.
///
/// Exposed so callers that need "is this a REAL heading, even one
/// `documents::evidence::classify_section` has no bucket for" (Certifications,
/// Licenses, Languages-spoken, Awards, Publications, Volunteer, Work History,
/// …) can ask without forking a second copy of the list — those headings are
/// all in [`SECTION_NAMES`], but `classify_section`'s six-variant
/// `SectionKind` has no arm for any of them and buckets them all as `Other`.
pub(crate) fn is_known_section_name(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    if SECTION_NAMES.contains(&lower.as_str()) {
        return true;
    }
    lower.split_once(" & ").is_some_and(|(left, right)| {
        SECTION_NAMES.contains(&left.trim()) && SECTION_NAMES.contains(&right.trim())
    })
}

/// Collapse a LETTER-SPACED heading back into words, or `None` when the line is
/// not one.
///
/// Designers set headings with wide tracking, and some PDF producers bake that
/// into the text layer as real spaces: a CV in the wild extracts its headings as
/// `S E L E C T E D   P R O J E C T S`. Every heading test in this file, in
/// `documents::evidence` and in `model::document::SectionId` is a
/// string match, so that line matches NOTHING — the section is invisible, its
/// projects never seed, its links are never collected, and it renders as body
/// text. (It is also an ATS hazard in its own right: wide tracking is exactly
/// what breaks text extraction for a real applicant-tracking parser.)
///
/// Single-character tokens are the signal. Word gaps survive as RUNS of two or
/// more spaces, so `S E L E C T E D   P R O J E C T S` restores as
/// `SELECTED PROJECTS` rather than one welded word. Requires at least four
/// single-character tokens, so an ordinary short line is never rewritten.
pub(super) fn despace_letterspaced(clean: &str) -> Option<String> {
    let clean = clean.trim();
    let mut tokens = 0usize;
    for token in clean.split_whitespace() {
        if token.chars().count() != 1 {
            return None;
        }
        tokens += 1;
    }
    if tokens < 4 {
        return None;
    }
    let words: Vec<String> = clean
        .split("  ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.split_whitespace().collect::<String>())
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

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

/// Check if all-caps text is likely a company/role name
fn is_likely_company_or_role(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    words.iter().any(|word| COMPANY_KEYWORDS.contains(word))
}

/// True when `s` contains a run of 4+ consecutive ASCII digits — a bare year
/// ("2021") or similar. The "no years" guard below needs this, not "some
/// digit character appears exactly 4 times anywhere in the string": that
/// crude check let a heading-shaped line carrying a genuine year (e.g.
/// "PROJECT 2021", no second date to trip `DATE_RE`'s range shape) through
/// misclassified as a section heading.
fn has_four_consecutive_ascii_digits(s: &str) -> bool {
    let mut run = 0;
    for c in s.chars() {
        if c.is_ascii_digit() {
            run += 1;
            if run == 4 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// The ALL-CAPS `SectionHeader` shape rule: fully uppercase, 4–60 chars, at
/// least 2 alphabetic characters, no run of 4+ consecutive digits (a "no
/// years" guard), not a likely company/role/acronym token
/// (`is_likely_company_or_role`), no date-range shape (`DATE_RE`), and no
/// `@`. This is what recognizes a locale's own ALL-CAPS heading
/// (`PERFIL`/`PROFILO`/`WERKERVARING`/…, or an English heading not literally
/// in [`SECTION_NAMES`] such as "PROFESSIONAL EXPERIENCE") without a
/// per-locale word list. `pub(crate)` — mirrored in TS by
/// `isAllCapsSectionHeading`
/// (`packages/prompts/src/generate/text/header-contact-line.ts`) and kept in
/// parity by the shared fixture `fixtures/all-caps-headings.json`.
pub(crate) fn is_all_caps_section_heading(clean: &str) -> bool {
    clean == clean.to_uppercase()
        && clean.len() >= 4
        && clean.len() <= 60
        && clean.chars().filter(|c| c.is_alphabetic()).count() >= 2
        && !has_four_consecutive_ascii_digits(clean)
        && !is_likely_company_or_role(clean)
        && !DATE_RE.is_match(clean)
        && !clean.contains('@')
}

/// A Markdown thematic break: 3+ identical `-`, `*`, or `_` markers (optionally
/// separated by spaces) and nothing else — e.g. `---`, `***`, `___`, `- - -`.
/// The model emits these as section separators, but every template already draws
/// its own section rules, so a literal break renders as stray "---" text AND
/// doubles the divider. Recognized here so it can be dropped as a blank line.
pub(super) fn is_thematic_break(line: &str) -> bool {
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
/// `pub(crate)` so `pipeline::resume::stages::sections::real_section_count`
/// can ask the same question `parse_line` already answered, rather than
/// re-deriving the ATX shape a second time.
pub(crate) fn strip_atx_heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some(line[hashes..].trim_start())
    } else {
        None
    }
}
