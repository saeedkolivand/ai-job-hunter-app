//! Internal consistency — the document contradicting itself or its source.
//!
//! All Warnings. Each of these has a legitimate explanation often enough
//! (a promotion, a role renamed for clarity, a skill genuinely used but not
//! written up) that a Critical would be wrong; what the user needs is a
//! pointer, not a block.

use std::collections::HashSet;

use crate::documents::evidence::{
    date_spans, function_words, has_curated_function_words, identity_tokens, years_in, SectionKind,
};
use crate::export::types::{LineKind, ParsedLine};

use super::factual::{MAX_SCANNED_ENTRIES, MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS};
use super::{
    issue, Analysis, ContentIssue, Section, CONSISTENCY_DATE_ORDER, CONSISTENCY_PROJECT_STRUCTURE,
    CONSISTENCY_SKILL_NOT_DEMONSTRATED, CONSISTENCY_TITLE_DRIFT,
};

/// Description lines a full project entry may carry after its stack line.
/// Beyond this the entry has stopped being a project card and become prose.
pub const MAX_PROJECT_DESCRIPTION_LINES: usize = 3;

/// `consistency.date_order` — a span whose start year is after its end year.
///
/// ## What counts as a span
///
/// Two years on a line are not a date span. This used to read EVERY line, take
/// any two years on it as `(start, end)` and report them as swapped when the
/// second was smaller — so an ordinary bullet measuring today against an older
/// baseline ("cut the incident count to 2024 levels from the 2019 baseline")
/// was accused of carrying a reversed date span.
///
/// So a pair must come from one of the two places a span actually lives:
///
/// * the parser's own date COLUMN (`right_text`, set only for the two-space
///   entry form) — a date by construction, months and all;
/// * otherwise a `2021 – 2018`-shaped fragment anywhere on the line
///   ([`date_spans`]), where the separator is what distinguishes a span from
///   two numbers that happen to share a sentence.
fn date_order_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = Vec::new();
    for section in &ctx.generated_sections {
        for line in &section.lines {
            let spans: Vec<(u32, u32, &str)> = match line.right_text.as_deref() {
                // Only a genuine two-ended column can be out of order; a single
                // year or a "2021 – Present" span has nothing to compare.
                Some(dates) => match years_in(dates)[..] {
                    [start, end] => vec![(start, end, dates)],
                    _ => Vec::new(),
                },
                None => date_spans(&line.text),
            };
            for (start, end, span) in spans {
                if start > end {
                    issues.push(issue(
                        CONSISTENCY_DATE_ORDER,
                        section.heading.as_deref(),
                        format!(
                            "This entry runs from {start} to {end} — the end date is before the \
                             start. Swap them."
                        ),
                        Some(span.trim().to_string()),
                    ));
                }
            }
        }
    }
    issues
}

/// `(company-token set, title)` for every employment entry in `sections`, up to
/// [`MAX_SCANNED_ENTRIES`].
///
/// The cap is here rather than at the call site because BOTH sides of
/// [`title_drift_issues`]' comparison come through this function, and the loop
/// it feeds is O(generated entries × source entries). Same rationale, same
/// constant, as `factual::dropped_role_issues` — imported rather than
/// re-declared so the two scans can never disagree about the bound.
fn titled_entries(sections: &[Section]) -> Vec<(HashSet<String>, String)> {
    sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .flat_map(|s| s.lines.iter())
        .filter(|l| matches!(l.kind, LineKind::JobEntry))
        .take(MAX_SCANNED_ENTRIES)
        .map(|l| {
            let label = l.text.as_str();
            // "Senior Engineer | Acme Corp | 2021 – Present" and
            // "Senior Engineer, Acme Corp" both put the title first.
            let (title, company) = label
                .split_once(['|', '·', '•', ','])
                .map(|(t, c)| (t.trim().to_string(), c.to_string()))
                .unwrap_or_else(|| (String::new(), label.to_string()));
            // Only tokens that name a SPECIFIC employer may match two entries to
            // each other, so the identity set is `documents::evidence`'s —
            // the same legal-form and geography exclusions `factual` decides
            // survival on. Sharing a "GmbH" (or a "Berlin") made two unrelated
            // employers the same employer, and the second entry's title was
            // then reported as drift from the first one's on a document that
            // was byte-identical to its source.
            //
            // Digits go on top of that, and only here: the split above keeps
            // the date span inside `company`, so two unrelated employers that
            // merely ended and began in the same year shared "2018".
            let tokens = identity_tokens(&company, MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS)
                .into_iter()
                .filter(|t| !t.chars().any(|c| c.is_ascii_digit()))
                .collect();
            (tokens, title)
        })
        .collect()
}

/// `consistency.title_drift` — the same employer, a different job title.
///
/// Matched on shared company tokens, compared on title tokens. Fires only when
/// the two titles share NO content token at all: "Senior Engineer" → "Staff
/// Engineer" is a promotion the candidate may have had, while "Senior Engineer"
/// → "Product Manager" is the drift worth surfacing.
fn title_drift_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source = titled_entries(&ctx.source_sections);
    let mut issues = Vec::new();
    for (company, title) in titled_entries(&ctx.generated_sections) {
        if title.trim().is_empty() || company.is_empty() {
            continue;
        }
        let Some((_, source_title)) = source
            .iter()
            .find(|(c, t)| !t.trim().is_empty() && !c.is_disjoint(&company))
        else {
            continue;
        };
        let generated_tokens = ctx.tokens(&title);
        let source_tokens = ctx.tokens(source_title);
        if generated_tokens.is_empty() || source_tokens.is_empty() {
            continue;
        }
        if generated_tokens.is_disjoint(&source_tokens) {
            issues.push(issue(
                CONSISTENCY_TITLE_DRIFT,
                Some("Experience"),
                format!(
                    "Your source résumé calls this role \"{source_title}\" but the generated \
                     document calls it \"{title}\". Use the title your employer actually gave you."
                ),
                Some(format!("{source_title} → {title}")),
            ));
        }
    }
    issues
}

/// How many words a `label:` head may run to before it stops being a category
/// label. "Programming languages:" is three; a sentence that merely contains a
/// colon ("Shipped three services: billing, ledger and search") is more, and its
/// head is content that must keep counting as a claim.
pub const MAX_SKILLS_LABEL_WORDS: usize = 3;

/// Separators a skills LIST is written with — the signal that what follows a
/// colon is a set of ITEMS rather than a grade for the word in front of it.
///
/// Two members are decided rather than obvious:
///
/// * `/` is **out**. "Deutsch: C1/C2" and "Python: 3/5" are single grades that
///   happen to carry a slash, while a category list is written with commas or
///   middots — a slash between two items is rare enough that reading it as a
///   list costs more than it buys.
/// * `•` is **in**. A bullet glyph is never part of a proficiency grade, so it
///   can only be separating items.
const SKILLS_LIST_SEPARATORS: &[char] = &[',', '·', ';', '|', '•'];

/// The half of a `head: tail` skills line that actually claims a skill.
///
/// Two different lines share that shape, and the non-skill half is the opposite
/// one in each:
///
/// * a CATEGORY label + its items — "Languages: Rust, Python",
///   "Programmiersprachen: Rust" — where the head names no skill. This is the
///   commonest skills layout there is, and "languages", "frameworks",
///   "databases" and "tooling" were all reported back as skills the résumé
///   never demonstrates.
/// * a SKILL + its proficiency grade — "Python: Advanced",
///   "Deutsch: Muttersprache" — where the TAIL names no skill. Stripping the
///   head here dropped the only real claim on the line (Python was never
///   checked at all) and kept the grade, so "advanced" was reported instead —
///   a finding about a word the user cannot demonstrate and would not want to.
///
/// The discriminator is the tail, and it is [`SKILLS_LIST_SEPARATORS`] alone: a
/// list is PUNCTUATED. Length is not a signal — a grade runs to as many words
/// as the language needs it to ("Englisch: verhandlungssicher in Wort und
/// Schrift" is the standard German Sprachen phrasing, "Python: 5 years in
/// production" its English equivalent), and counting words made every one of
/// those a list, so the grade words became the claims and the skill in front of
/// the colon was never checked at all. Only a SHORT head is considered a label
/// at all ([`MAX_SKILLS_LABEL_WORDS`]), and only the first colon — a longer
/// head is prose that happens to carry a colon, and dropping it would silently
/// stop policing everything in front of it.
///
/// **The boundary this knowingly gets wrong**, in one direction only: a
/// category whose items carry no separator reads as a grade, so the items are
/// not policed. That covers both a one-item category ("Frameworks: React") and
/// an unpunctuated multi-item one ("Tools: Docker Kubernetes Terraform"). It is
/// the cheaper error: those two layouts are uncommon, while a multi-word grade
/// is the normal way to write a language level, and the alternative reports
/// words like "verhandlungssicher" or "advanced" as skills the résumé fails to
/// demonstrate — a finding about a word the user cannot act on.
///
/// What that boundary must NOT do is report the LABEL in the items' place. A
/// category word is demonstrated by an experience section roughly never, so
/// "Frameworks: React" answered a missed check with a false one: "frameworks is
/// listed under skills but never appears in your experience". So when the grade
/// reading wins and the head is a [`SKILLS_CATEGORY_LABELS`] word, the line
/// claims NOTHING — the miss stays a miss instead of becoming an accusation.
fn strip_skills_label(line: &str) -> &str {
    let Some((head, tail)) = line.split_once(':') else {
        return line;
    };
    if super::word_count(head) > MAX_SKILLS_LABEL_WORDS {
        return line;
    }
    if tail.contains(SKILLS_LIST_SEPARATORS) {
        tail
    } else if is_category_label(head) {
        ""
    } else {
        head
    }
}

/// Words a skills line uses to NAME a category rather than to claim a skill,
/// `en` + `de` — the two languages this crate curates vocabulary for anywhere.
///
/// Matched as a SUBSTRING of a head token, which is what makes one entry cover
/// a language's inflections and compounds at once: `sprach` covers "Sprachen"
/// and "Programmiersprachen", `datenbank` covers "Datenbanken", `tool` covers
/// "Tools" and "Tooling". Deliberately tiny — every entry here is a skill this
/// check stops policing, so the bar is "nobody lists this as a skill".
const SKILLS_CATEGORY_LABELS: &[&str] = &[
    "language",
    "sprach",
    "framework",
    "library",
    "bibliothek",
    "tool",
    "werkzeug",
    "database",
    "datenbank",
    "cloud",
    "skill",
    "kenntnis",
    "fähigkeit",
    "kompetenz",
    "technolog",
    "stack",
    "methode",
];

/// Whether `head` names a CATEGORY: any of its words carries a
/// [`SKILLS_CATEGORY_LABELS`] stem.
///
/// Any word, not all of them, so a qualified label ("Everyday programming
/// languages", "Weitere Kenntnisse") still reads as one. The cost is that a real
/// skill whose name contains a category word ("Cloud Architecture: Advanced")
/// stops being policed — a miss, which is the direction this whole family errs
/// in.
fn is_category_label(head: &str) -> bool {
    head.split_whitespace().any(|word| {
        let lower = word.to_lowercase();
        SKILLS_CATEGORY_LABELS.iter().any(|c| lower.contains(c))
    })
}

/// `consistency.skill_not_demonstrated` — a skill listed in the skills section
/// that no experience or project line backs up.
///
/// The ATS argument for listing a skill is real, so this is advice, not a
/// block: it tells the user which claims an interviewer will have nothing to
/// ask about.
///
/// ## Only for a language whose non-skills are known
///
/// The claimed set is every token on a skills line, so the finding is only as
/// good as the filter that removes the words which are not skills — and that
/// filter is [`function_words`], which is curated for `en` (through the
/// kernel's own `STOPWORDS`) and `de` and for nothing else. An empty list means
/// "no filter", not "nothing to filter": on a French résumé "Connaissances
/// approfondies en Rust et Python" reported *connaissances* and *approfondies*
/// as skills the candidate fails to demonstrate.
///
/// So the whole check goes quiet for an uncurated language, exactly as
/// `ats::keyword_density_issues` does and for the same reason: a conclusion that
/// depends on function words having been removed may not be drawn where they
/// were not. This suppresses REAL gaps too (a French skills line listing
/// something the experience never shows) — accepted, because this family is
/// advisory while a wrong finding costs the user's trust in the panel, and
/// because there is no way here to tell the two apart. Adding a list to
/// `function_words` re-enables it, in one place, for every surface at once.
fn skill_not_demonstrated_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !has_curated_function_words(&ctx.lang) {
        return Vec::new();
    }
    let Some(skills) = ctx.section_of_kind(SectionKind::Skills) else {
        return Vec::new();
    };
    let demonstrated: HashSet<String> = ctx
        .generated_sections
        .iter()
        .filter(|s| matches!(s.kind, SectionKind::Experience | SectionKind::Projects))
        .flat_map(|s| s.lines.iter())
        .flat_map(|l| ctx.tokens(&l.text))
        .collect();
    if demonstrated.is_empty() {
        return Vec::new(); // Nothing to check against — a skills-only document.
    }

    let claimed: HashSet<String> = skills
        .lines
        .iter()
        .flat_map(|l| ctx.tokens(strip_skills_label(&l.text)))
        .collect();
    // Tokens are STEMMED when the languages align, so map every one back to a
    // readable form before it reaches the user: "kubernet is listed under
    // skills" is a finding nobody can act on. Sorted on the display form so the
    // report reads alphabetically as printed.
    //
    // The same display form is what the function-word filter reads, for the
    // same reason `documents::evidence` filters its skills split on it: the list
    // holds words, not stems. It is the second half of the label fix — a line
    // that spells its category out ("Kenntnisse in Rust und Python") carries the
    // filler inline, where no `label:` rule can reach it.
    let stop = function_words(&ctx.lang);
    let mut missing: Vec<String> = claimed
        .difference(&demonstrated)
        .map(|token| ctx.display(token))
        .filter(|display| !stop.contains(&display.as_str()))
        .collect();
    missing.sort(); // Deterministic: the sets above are unordered.
    missing
        .into_iter()
        .map(|skill| {
            issue(
                CONSISTENCY_SKILL_NOT_DEMONSTRATED,
                Some("Skills"),
                format!(
                    "\"{skill}\" is listed under skills but never appears in your experience or \
                     projects. Add the line that proves it, or drop the claim."
                ),
                Some(skill),
            )
        })
        .collect()
}

/// Which of the three owner-locked project shapes an entry has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectTier {
    /// `**Name** · app-link · repo-link`, a `·`-separated stack line, and 1–3
    /// description lines.
    Full,
    /// Name + links + stack line, no description — the source had no blurb.
    NameLinksStack,
    /// `• Name · Website · Github` on one line — the source had links only.
    Compact,
}

/// Group a projects section's non-blank lines into entries.
///
/// An entry begins at a line that names a project: a bold run (`**Name**`) or a
/// bullet. Everything up to the next such line belongs to it.
///
/// `pub(super)` because `factual::project_links` needs the same grouping to know
/// which line is the stack line — two graders disagreeing about where an entry
/// starts is how a structure warning and a link Critical contradict each other.
pub(super) fn project_entries(section: &Section) -> Vec<Vec<&ParsedLine>> {
    let mut entries: Vec<Vec<&ParsedLine>> = Vec::new();
    for line in section
        .lines
        .iter()
        .filter(|l| !matches!(l.kind, LineKind::Blank) && !l.text.trim().is_empty())
    {
        let starts_entry =
            matches!(line.kind, LineKind::Bullet) || line.segments.iter().any(|s| s.bold);
        if starts_entry || entries.is_empty() {
            entries.push(vec![line]);
        } else if let Some(last) = entries.last_mut() {
            last.push(line);
        }
    }
    entries
}

/// The owner-locked degradation ladder: full → name+links+stack → compact.
/// Anything else is a structure the templates were not designed around.
fn tier_of(entry: &[&ParsedLine]) -> Option<ProjectTier> {
    let title = entry.first()?;
    let separators = ['·', '|', '•'];
    match entry.len() {
        // Compact: everything on the title line, `Name · Website · Github`.
        1 => title
            .text
            .contains(separators)
            .then_some(ProjectTier::Compact),
        // Title + stack line.
        2 => entry[1]
            .text
            .contains(separators)
            .then_some(ProjectTier::NameLinksStack),
        // Title + stack + 1..=MAX description lines.
        n if n <= 2 + MAX_PROJECT_DESCRIPTION_LINES => entry[1]
            .text
            .contains(separators)
            .then_some(ProjectTier::Full),
        _ => None,
    }
}

/// `consistency.project_structure` — a project entry outside the three
/// accepted shapes.
fn project_structure_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let Some(section) = ctx.section_of_kind(SectionKind::Projects) else {
        return Vec::new();
    };
    project_entries(section)
        .into_iter()
        .filter(|entry| tier_of(entry).is_none())
        .map(|entry| {
            let name = entry.first().map(|l| l.text.clone()).unwrap_or_default();
            issue(
                CONSISTENCY_PROJECT_STRUCTURE,
                Some("Projects"),
                format!(
                    "The project \"{name}\" does not follow one of the three supported layouts \
                     (name + links + stack + up to {MAX_PROJECT_DESCRIPTION_LINES} description \
                     lines, name + links + stack, or the one-line compact form). It may render \
                     unevenly next to the others."
                ),
                Some(name),
            )
        })
        .collect()
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = date_order_issues(ctx);
    issues.extend(title_drift_issues(ctx));
    issues.extend(skill_not_demonstrated_issues(ctx));
    issues.extend(project_structure_issues(ctx));
    issues
}
