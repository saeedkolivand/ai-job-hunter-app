//! Dropping the benefits / "what we offer" sections of a job ad before keyword extraction
//! (issue #1554): perks such as "eigene Hardware" or "Jobrad" are what the employer gives, never
//! what the candidate must bring, so they must not become ATS keywords or requirements.

/// Headings (lowercase, no trailing colon) that open a benefits section.
const BENEFIT_HEADINGS: &[&str] = &[
    "benefits",
    "perks",
    "perks & benefits",
    "perks and benefits",
    "what we offer",
    "we offer",
    "what you get",
    "what's in it for you",
    "our offer",
    "wir bieten",
    "wir bieten dir",
    "wir bieten ihnen",
    "das bieten wir",
    "das bieten wir dir",
    "das bieten wir ihnen",
    "was wir bieten",
    "was wir dir bieten",
    "was wir ihnen bieten",
    "unser angebot",
    "unsere benefits",
    "deine benefits",
    "ihre benefits",
    "deine vorteile",
    "ihre vorteile",
    "ce que nous offrons",
    "nous offrons",
    "nous vous offrons",
    "avantages",
    "lo que ofrecemos",
    "ofrecemos",
    "beneficios",
    "cosa offriamo",
    "offriamo",
    "o que oferecemos",
    "oferecemos",
    "benefícios",
    "wat wij bieden",
    "wij bieden",
];

/// Headings that close a benefits section (the ad moves on to another topic).
const OTHER_HEADINGS: &[&str] = &[
    "requirements",
    "qualifications",
    "responsibilities",
    "your profile",
    "your tasks",
    "your skills",
    "what you bring",
    "what you'll do",
    "what you will do",
    "what we're looking for",
    "what we are looking for",
    "who you are",
    "das bringst du mit",
    "das bringen sie mit",
    "was du mitbringst",
    "was sie mitbringen",
    "deine qualifikationen",
    "ihre qualifikationen",
    "about us",
    "about you",
    "the role",
    "how to apply",
    "ihre aufgaben",
    "deine aufgaben",
    "aufgaben",
    "dein profil",
    "ihr profil",
    "anforderungen",
    "über uns",
    "bewerbung",
    "exigences",
    "profil recherché",
    "requisitos",
    "requisiti",
];

fn heading_text(line: &str) -> String {
    line.trim()
        .trim_matches(|c: char| c == '#' || c == '*' || c == ':' || c.is_whitespace())
        .to_lowercase()
}

fn is_markdown_heading(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('#') || (t.len() > 4 && t.starts_with("**") && t.ends_with("**"))
}

/// Remove every benefits section: from a benefits heading up to the next heading (markdown
/// heading, bold line, or a known section heading like "Ihr Profil"). Run on raw description
/// text, before [`super::markdown_to_plain`] strips the heading markers.
pub(super) fn strip_benefit_sections(text: &str) -> String {
    let mut skipping = false;
    let mut kept: Vec<&str> = Vec::new();
    for line in text.lines() {
        let h = heading_text(line);
        if BENEFIT_HEADINGS.contains(&h.as_str()) {
            skipping = true;
            continue;
        }
        if skipping && (OTHER_HEADINGS.contains(&h.as_str()) || is_markdown_heading(line)) {
            skipping = false;
        }
        if !skipping {
            kept.push(line);
        }
    }
    kept.join("\n")
}
