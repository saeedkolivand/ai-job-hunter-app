//! Line-shape heuristics: dates, contact lines, project tech-stack / title
//! lines, and the generic "entry title" shape — all markdown-stripped, regex
//! and punctuation/length rules, no section-name knowledge (see [`super::headings`]
//! for that half).

use regex::Regex;
use std::sync::LazyLock;

use super::headings::{is_all_caps_section_heading, is_known_section_name};

// Lazy-initialized regexes for performance. The `[\s\S]{0,30}` middle span is
// GREEDY (not `{0,30}?`), so a `.find()` on a "Mon YYYY – Mon YYYY" RANGE
// captures through the second (end) date, not the first one — the year
// immediately after the opening month is itself a valid END alternative
// (`20\d{2}`), so a lazy quantifier would stop there and truncate the match to
// just "Mon YYYY". Greedy vs. lazy only changes which SPAN is reported when
// several are possible; it recognizes the identical set of strings, so this
// is a no-op for every existing `.is_match()` call site (there is no other
// `.find()`/span-based use of this regex) — verified, not merely asserted.
pub(super) static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?|19\d{2}|20\d{2})[\s\S]{0,30}(?:Present|Current|Now|Heute|Ongoing|Actuel|20\d{2}|19\d{2})\b").unwrap()
});

// A pipe/middot segment that IS a standalone single date — a bare year or
// "Month YYYY" (e.g. "2021", "Jan 2021"). Anchored so it matches only when the
// whole segment is a date. Lets single-year entries (common for PROJECTS and
// education) be recognized as entries, not only date ranges.
pub(super) static SOLO_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\.?\s+)?(?:19|20)\d{2}\s*$").unwrap()
});

pub(super) static BULLET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([•\-–*·▪▸►✓✔○●◆◇■□▹▸]|\d+\.|[a-z]\))\s+(.+)$").unwrap());

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+?\d[\d\s\-().]{7,}").unwrap());

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)linkedin\.com|github\.com|portfolio|website|^https?://").unwrap()
});

/// A phone number or a known contact-platform / URL-scheme match, with no
/// separator-count or `@` arm — the segment-level test the pipe/middot
/// job-entry-vs-contact disambiguation in `parse_line` needs (a WHOLE-line
/// `is_contact_shaped` would itself require 2+ separators, which a single
/// segment never has).
pub(super) fn is_contactish_segment(s: &str) -> bool {
    PHONE_RE.is_match(s) || URL_RE.is_match(s)
}

/// Combined `|`/`·`/`•` separator count — shared by the job-entry
/// pipe/middot-with-date check and [`is_contact_shaped`] below.
pub(super) fn separator_count(clean: &str) -> usize {
    clean.matches('|').count() + clean.matches('·').count() + clean.matches('•').count()
}

/// The Contact-line shape test: an `@`, a phone number, ≥2 pipe/middot/bullet
/// separators, or a known contact-platform keyword / bare `http(s)://` URL.
/// `pub(crate)` as the single source of truth for what counts as a résumé's
/// contact line — mirrored in TS by `isHeaderContactLine`
/// (`packages/prompts/src/generate/text/header-contact-line.ts`). A
/// shared-fixture parity test (`fixtures/header-contact-line.json`, read by
/// both `cargo test export::parser` and that file's TS test) keeps the two
/// from silently drifting.
pub(crate) fn is_contact_shaped(clean: &str) -> bool {
    clean.contains('@')
        || PHONE_RE.is_match(clean)
        || separator_count(clean) >= 2
        || URL_RE.is_match(clean)
}

/// The project TECH-STACK line shape: the `·`-separated technologies line that
/// sits directly under a project's bold title in the locked project signature
/// (`pipeline::resume::project_render::render_project`).
///
/// Deliberately looser than [`is_contact_shaped`] on separator count — a
/// two-item stack (`Rust · SQLite`) has only ONE separator and would otherwise
/// fall through to `Text` — and tighter on everything else, because the only
/// other thing that can appear in that slot is the project's prose description.
/// Terminal sentence punctuation and length are what separate the two: a stack
/// is a short list, a description is a sentence. The `@` arm keeps a stray
/// contact line out.
///
/// Shape-only, and NOT section-aware: the caller
/// (`crate::model::adapter`) applies it exclusively inside a
/// [`SectionId::Projects`](crate::model::document::SectionId) section and only
/// for the line immediately under an entry title, so a `·`-bearing line
/// anywhere else is unaffected.
pub(crate) fn is_project_stack_shaped(clean: &str) -> bool {
    let clean = clean.trim();
    !clean.is_empty()
        && separator_count(clean) >= 1
        && !is_contactish_for_stack(clean)
        && !clean.ends_with(['.', '!', '?', ':'])
        && clean.chars().count() <= 120
}

/// Does this line look like a project TITLE purely by SHAPE — no markdown?
///
/// The generated project signature marks its title with a bold run, and a
/// hand-written one often uses a bullet. Neither survives IMPORT: PDF and DOCX
/// extraction keeps the words and drops the styling, so a candidate's own CV
/// carrying a perfectly-formed project block has no `**` and no `•` anywhere.
///
/// Three conditions, and all three are load-bearing:
///
/// * `at_paragraph_start` — the line opens a paragraph (first content line of
///   the section, or preceded by a blank). Entries are separated by blank lines
///   and a description line never is, which is what stops an unpunctuated line
///   INSIDE an entry from hijacking a following stack line: `Used by 200 teams`
///   above a second `Go · gRPC · Redis` line is prose, not a new project.
/// * The next line is a technology stack — prose does not sit directly above a
///   `·`-separated list. Callers must not look past the end of the section for
///   it, or a separator-bearing HEADING (`SKILLS · TOOLS`) makes a title out of
///   the last line of Projects.
/// * The line is short, is not a sentence, and is not a stack itself — or a
///   two-stack sequence would open an entry on the second one.
///
/// KNOWN LIMIT: a section whose entries are not blank-separated, or that has no
/// stack lines at all, has no shape signal and is left to the markdown rules.
/// Pinned by tests; widening it trades false negatives for false positives on a
/// candidate's real prose, which is the worse failure.
///
/// `pub(crate)` and deliberately SHARED: `model::adapter` groups a section into
/// render entries with it and `validate::content::project_entry_starts` groups
/// the same section for the seeder, the normalizer and the tier grader. A second
/// answer to "where does an entry begin" would shift every following line by one
/// — turning a stack line into a description and a truthful document into a
/// `consistency.project_structure` warning.
pub(crate) fn is_project_title_shaped(
    clean: &str,
    next_clean: Option<&str>,
    at_paragraph_start: bool,
) -> bool {
    let clean = clean.trim();
    at_paragraph_start
        && !clean.is_empty()
        && clean.chars().count() <= 100
        && !clean.ends_with(['.', '!', '?'])
        && !is_project_stack_shaped(clean)
        && next_clean.is_some_and(is_project_stack_shaped)
}

/// Contact-ish content that must never be read as a technology list: an email,
/// a phone number, a URL scheme or markdown link, or a known contact host.
///
/// NOT [`is_contact_shaped`]: that treats ≥ 2 separators as contact-shaped, which
/// is exactly what a three-item technology stack looks like, so reusing it would
/// reject the common case outright.
///
/// [`URL_RE`] alone is not enough either — it only knows the contact platforms
/// and a scheme ANCHORED at the start of the line, so a trailing link like
/// `Demo · https://example.dev` slips past it. The unanchored `://` / `](` /
/// `www.` tests are what actually catch a link sitting after a separator.
///
/// A bare-domain test is deliberately absent: `Node.js · socket.io · Express` is a
/// real stack, and every heuristic that catches `demo.example.dev` also catches
/// `socket.io`. A bare-domain link line is left accepted rather than corrupting
/// a genuine stack — the failure it causes is cosmetic, the other is not.
fn is_contactish_for_stack(clean: &str) -> bool {
    clean.contains('@')
        || clean.contains("://")
        || clean.contains("](")
        || clean.to_ascii_lowercase().contains("www.")
        || PHONE_RE.is_match(clean)
        || URL_RE.is_match(clean)
}

/// The line-0-ONLY Contact test — narrower than [`is_contact_shaped`]: just an
/// `@` or a phone shape, with no pipe/URL arms. This is what decides Name vs
/// Contact for the résumé's first line (`parse_line`'s `idx == 0` case) — a
/// combined "Jane Doe | jane@example.com" is classified `Contact`, not `Name`,
/// there, so `header.name` comes out empty for that input shape regardless of
/// what runs downstream. `pub(crate)` for the same reason as
/// `is_contact_shaped`: mirrored in TS (`isFirstLineContactShaped` in the same
/// `header-contact-line.ts`) and kept in parity by the same shared fixture's
/// `firstLine` field.
pub(crate) fn is_first_line_contact_shaped(clean: &str) -> bool {
    clean.contains('@') || PHONE_RE.is_match(clean)
}

/// A short, non-prose "title" line — 1–12 words, ≤100 chars, no terminal
/// sentence punctuation, and NOT itself heading-shaped (a known section name
/// or an ALL-CAPS heading) — the shape of a job/entry title or "Title ·
/// Company" line, never a prose paragraph or a section heading. Backs the
/// paired next-line-date [`LineKind::JobEntry`] branches in [`parse_line`]:
/// the two-space/paren/pipe job-entry patterns above all require the date on
/// the SAME line as the title; this covers the shape they don't — "Title ·
/// Company" \n "Mon YYYY – Mon YYYY[, Location]" on its own following line
/// (common LinkedIn-export / AI-generated résumé layout). The heading
/// exclusion matters for the BACKWARD half of that pair (checked against the
/// PREVIOUS line's re-derived shape, not its already-decided `LineKind`): a
/// bare section heading ("Certifications") is itself short and
/// unpunctuated, and without this guard a heading immediately followed by a
/// leading-date entry line ("2023 AWS Certified …") would wrongly treat the
/// heading as an opened entry and silently drop the date. Mirrors (loosely)
/// `model::adapter::is_title_like`, which lives one layer up and this module
/// cannot depend on.
pub(super) fn is_entry_title_shaped(clean: &str) -> bool {
    let words = clean.split_whitespace().count();
    (1..=12).contains(&words)
        && clean.chars().count() <= 100
        && !clean.ends_with(['.', '!', '?'])
        && !is_known_section_name(clean)
        && !is_all_caps_section_heading(clean)
        // A header contact line is title-SHAPED by every other measure here —
        // short, unpunctuated, not a heading — so without this it is eligible to
        // open an entry, and the `is_contact_shaped` branch that would have
        // claimed it runs LATER in `parse_line`. A contact line followed by a
        // leading-date line then becomes a fabricated job entry: the details
        // vanish from the header and resurface as a job title. Every sibling
        // job-entry branch already guards on this; this one has to as well.
        && !is_contact_shaped(clean)
}
