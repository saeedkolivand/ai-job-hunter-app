//! The date-span and date-column shape tests behind the entry splitter.
//!
//! Split out of `evidence/entry.rs` (R8's hard LOC cap); everything here moved
//! verbatim. This is the family that answers "is this text a DATE?": a closed span,
//! an open-ended one, a whole date column, or just a year — decided in one place so
//! every surface (`validate::content` included) agrees.

use std::sync::LazyLock;

use regex::Regex;

/// The lowercased alphanumeric runs in `text`, in order.
pub(super) fn word_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Present-tense markers a date span can end with, in the languages the résumé
/// pipeline supports.
///
/// Matched only through [`PRESENT_MARKER_ADJACENT_RE`] — a year immediately in
/// front of the marker — never as a bare word search over arbitrary text.
/// Word boundaries alone ([`contains_word`]) fixed the SUBSTRING failure
/// (`present` inside "presented", `now` inside "knowledge"), but every entry
/// here is ALSO an ordinary word on its own (`current`, `ongoing`, `now`,
/// `actual`…), so a bare word-boundary search over a whole bullet still read
/// "Reduced actual costs by 20% in 2023" and "...while keeping the ongoing
/// migration on schedule" as date contexts and, downstream, produced a false
/// `factual.unsupported_date` Critical on a truthful bullet. Requiring
/// adjacency to a year is what a standalone word search cannot express.
pub const PRESENT_MARKERS: &[&str] = &[
    "present", "current", "now", "ongoing", "heute", "aktuell", "laufend", "actuel", "actual",
    "attuale", "heden", "atual",
];

/// Openers that make a span open-ended without naming a present-tense word:
/// `since 2021`, `seit 2021`, `from 2021`. Matched only when a year follows
/// immediately (optionally through a month), so the ordinary English
/// preposition in "cut costs from 2019 baselines" is not read as a date span.
static OPEN_ENDED_OPENER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:since|seit|from|ab|depuis|desde|dal|vanaf|sinds)\s+(?:\p{L}+\.?\s+)?(?:19|20)\d{2}\b")
        .unwrap()
});

/// A [`PRESENT_MARKERS`] word immediately after a year, with nothing between
/// but a span separator (`-`, `to`, `bis`, …): `2021 – Present`, `2019 to
/// Present`, `2016-Present`. This is the structural signal that tells a
/// genuine date span apart from a present-tense word sitting many words away
/// from an unrelated year — see [`PRESENT_MARKERS`]'s doc for the false
/// positives an unanchored word search produced. Built from [`PRESENT_MARKERS`]
/// so the regex vocabulary can never drift from the word list it matches.
static PRESENT_MARKER_ADJACENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let markers = PRESENT_MARKERS.join("|");
    Regex::new(&format!(
        r"(?i)\b(?:19|20)\d{{2}}\s*(?:[-–—/]|\bto\b|\bbis\b|\buntil\b|\bhasta\b|\bau\b|\bà\b|\ba\b|\btot\b|\bfino\b|\baté\b)\s*(?:{markers})\b"
    ))
    .unwrap()
});

/// True when `needle` (lowercase) occurs in `haystack` (lowercase) at word
/// boundaries on both ends — so `vital` does not fire on `revitalize` and
/// `not just` does not fire inside `cannot justify`.
///
/// One boundary rule for every lexicon-style match in the résumé pipeline:
/// `validate::content` re-exports this as `contains_phrase`, and
/// [`PRESENT_MARKERS`] is compared through it on both surfaces.
pub fn contains_word(haystack_lower: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    haystack_lower.match_indices(needle_lower).any(|(i, m)| {
        let before = haystack_lower[..i].chars().next_back();
        let after = haystack_lower[i + m.len()..].chars().next();
        before.is_none_or(|c| !is_word(c)) && after.is_none_or(|c| !is_word(c))
    })
}

/// True when `s` names a span with **no end date**.
///
/// Three shapes, because résumés spell "still there" in more than one way and a
/// validator that only knows `Present` treats every other spelling as a closed
/// span:
///
/// 1. a present-tense marker right after a year (`2021 – Present`, `2021 –
///    Heute`) — [`PRESENT_MARKER_ADJACENT_RE`];
/// 2. an open-ended opener with a year (`since 2021`, `seit 2021`, `from 2021`);
/// 3. a trailing dash with a year in front of it (`2021 –`).
///
/// **A bare marker with no year anywhere in `s` is not open-ended.** `s` must
/// carry a year before any other branch is even tried — the same non-answer
/// [`DATE_ONLY_MARKERS`] already gives for a bare `"Today"`. A whole-line
/// caller asking whether a SOURCE résumé shows an ongoing role needs the year
/// to already be on that line; a caller holding only a bare fragment (no year
/// to anchor a marker to) has no date span to name one way or the other.
pub fn is_open_ended(s: &str) -> bool {
    if years_in(s).is_empty() {
        return false;
    }
    PRESENT_MARKER_ADJACENT_RE.is_match(s)
        || OPEN_ENDED_OPENER_RE.is_match(s)
        || s.trim_end().ends_with(['-', '–', '—'])
}

/// True when `s` carries a year (1900–2099) — the shape a date span has.
/// Shared with the content validators so "what counts as a date" is decided
/// in one place.
///
/// **Not `|| is_open_ended(s)` — that disjunct is dead.** [`is_open_ended`]
/// requires a year in `s` before any of its own branches run (see its doc),
/// so it can never be true while `years_in(s)` is empty, and `years_in(s)`
/// being non-empty already makes this function true on its own. ORing it in
/// therefore can never change the result; it used to, back when
/// `is_open_ended`'s first branch matched a bare present-tense marker with no
/// year anywhere.
pub fn looks_like_date_span(s: &str) -> bool {
    !years_in(s).is_empty()
}

/// A CLOSED span: two years with nothing between them but a span separator and
/// at most one month-shaped token (`2018 – 2021`, `Jan 2018 - Mar 2021`,
/// `2018 to 2021`, `05/2018 – 07/2021`).
///
/// The separator is what makes this a SPAN rather than two numbers that happen
/// to share a line. Same year window as [`years_in`], for the same reason it
/// lives in this module: what counts as a date is decided in one place.
static DATE_SPAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:19|20)\d{2}\s*(?:[-–—]|\bto\b|\bbis\b|\buntil\b)\s*(?:[\p{L}\d]{1,9}[./]?\s*)?(?:19|20)\d{2}",
    )
    .unwrap()
});

/// Every closed date span in `s`, as `(start_year, end_year, span_text)` — the
/// text is what a finding may quote as evidence.
///
/// `validate::content::consistency` used to take ANY two years on a line as a
/// span and report the pair as swapped when the second was smaller. A bullet
/// that measures this year against an older baseline ("cut incidents to 2024
/// levels from the 2019 baseline") therefore read as an entry whose end date
/// preceded its start. Requiring the separator is what tells a date column
/// apart from prose; a pair with a word between the years is not a span.
pub fn date_spans(s: &str) -> Vec<(u32, u32, &str)> {
    DATE_SPAN_RE
        .find_iter(s)
        .filter_map(|m| match years_in(m.as_str())[..] {
            [start, end] => Some((start, end, m.as_str())),
            // The optional month slot can swallow a third year; a fragment that
            // does not resolve to exactly two is not a span this can judge.
            _ => None,
        })
        .collect()
}

/// Month names and abbreviations, English and German, matched WHOLE against a
/// [`word_tokens`] token.
///
/// Deliberately not a prefix list: `mar` as a prefix also matches "marketing",
/// and this list's whole job is to decide that a fragment carries nothing but a
/// date. Deliberately only the two languages the rest of this pipeline curates
/// — a French or Spanish month simply leaves [`is_date_only`] false, which
/// costs an entry line its attribution (the pre-existing behaviour) rather than
/// mis-reading a sentence as a date column.
const MONTH_TOKENS: &[&str] = &[
    "jan",
    "january",
    "januar",
    "feb",
    "february",
    "februar",
    "mar",
    "march",
    "mär",
    "märz",
    "apr",
    "april",
    "may",
    "mai",
    "jun",
    "june",
    "juni",
    "jul",
    "july",
    "juli",
    "aug",
    "august",
    "sep",
    "sept",
    "september",
    "oct",
    "october",
    "okt",
    "oktober",
    "nov",
    "november",
    "dec",
    "december",
    "dez",
    "dezember",
];

/// "Today"/"currently"-family present-tense markers recognised **only** by
/// [`is_date_only`] — deliberately never added to [`PRESENT_MARKERS`], which
/// [`is_open_ended`] and `validate::content::factual::employment::unsupported_date_issues`
/// both also consult (the latter through [`trailing_date_column`]), and never
/// merged with either by stem/prefix matching.
///
/// **Why a separate list at all — measured, not just cautious.** Both
/// consumers now require date STRUCTURE — a year adjacent to the marker, or a
/// parsed `JobEntry`/comma-tail date column — not just the word's presence.
/// That structural requirement is what this list has never needed: every
/// spelling here is ordinary prose in its language, not just "today":
/// "currently" ("we are currently migrating"), "derzeit", "actuellement",
/// "presente" (also an ordinary Spanish/Portuguese/Italian adjective — "el
/// problema presente"). Putting any of them in [`PRESENT_MARKERS`] would let
/// `is_open_ended` treat a bare occurrence next to an unrelated year as an
/// open span. [`is_date_only`] cannot make that mistake even without the
/// adjacency requirement: it already requires EVERY token on the line to be a
/// digit, a month or a marker, so a prose sentence carrying any other word is
/// rejected before this list is ever consulted — exact-token equality against
/// a whole date column is structurally safe in a way word-bounded matching
/// against free text was not. (`is_open_ended`'s marker branch used to match
/// a bare word anywhere in arbitrary text — no year, no separator required —
/// which read "Reduced actual costs by 20% in 2023" as an open-ended span on
/// the word `actual` alone; fixed by requiring [`PRESENT_MARKER_ADJACENT_RE`]
/// instead of a bare [`contains_word`] search.)
///
/// **Why literal spellings, never a shared stem.** `actual`/`actuel`/
/// `actualidad`/`atualmente`/`attualmente` all share a Latin root and a
/// prefix rule would collapse them into one entry, but this const is already
/// read by a whole-line gate today and nothing stops a future caller reading
/// it against free text the way [`PRESENT_MARKERS`] used to be — the exact
/// hazard the paragraph above documents. A literal list stays safe under a
/// change of consumer; a prefix rule would not, so every spelling is listed
/// in full even where it costs a near-duplicate entry.
///
/// **`aujourd'hui`** is split by [`word_tokens`] into `aujourd` and `hui` —
/// the apostrophe is not alphanumeric — so both halves are listed;
/// `is_date_only` requires every token to resolve, not the phrase as a whole.
///
/// **`nu`** (Dutch/Swedish "now") is deliberately left OUT, not an oversight.
/// Every other entry here is 4+ letters; `nu` is two, which collides with
/// initials, unit abbreviations and stray two-letter tokens far more readily
/// than the rest of this list — and unlike those, a false hit doesn't need a
/// second unrelated word nearby, just one line where every OTHER token also
/// happens to be date-shaped. The Dutch/Swedish market share this pipeline
/// serves does not justify that exposure; `heden` (Dutch "today", already in
/// [`PRESENT_MARKERS`]) covers the same case at four letters with none of the
/// risk.
///
/// **Known boundary, not an oversight: single tokens only.** A multi-word
/// present-tense column — Spanish `en la actualidad`, French `en cours` /
/// `à ce jour`, Italian `ad oggi` / `in corso`, Portuguese `até hoje`,
/// Spanish `o presente`, Dutch `tot heden`, German `bis heute` (the last
/// already failing today, since `heute` is a [`PRESENT_MARKERS`] single
/// token) — is not recognised, because [`is_date_only`]'s `date_words` gate
/// asks whether EVERY token resolves on its own; a phrase needs the SEPARATOR
/// between its words to also be accounted for, which is a different
/// mechanism (multi-token phrase matching) and a wider change than this list.
///
/// **A second safety property, beyond "prose fails the token gate."** No
/// entry here is ever added to [`PRESENT_MARKERS`], so [`is_open_ended`]
/// never fires on any of these words — which means even a line built
/// ENTIRELY from digits/months/[`DATE_ONLY_MARKERS`] tokens still can't pass
/// [`is_date_only`] unless it also carries a real year or a genuine
/// [`PRESENT_MARKERS`] open-ended marker: [`looks_like_date_span`] gates on
/// exactly those two things and never consults this list.
/// `"Today Today"`, `"Hoy Hoy"`, `"Today 5"` and `"Currently 100"` all fail
/// [`is_date_only`] for this reason, not because any token is unrecognised —
/// every token in each of them resolves fine, but none of the four ever
/// reaches a year or a [`PRESENT_MARKERS`] marker. The disjointness this
/// relies on — no spelling lives in both lists — is asserted in the test
/// suite, not just documented here.
pub(super) const DATE_ONLY_MARKERS: &[&str] = &[
    "today",
    "aujourd",
    "hui",
    "oggi",
    "actualidad",
    "actualmente",
    "presente",
    "currently",
    "hoje",
    "atualmente",
    "hoy",
    "actuellement",
    "attualmente",
    "derzeit",
    "vandaag",
];

/// True when `s` is a date COLUMN and nothing else: every word in it is a
/// number, a month, a present-tense marker or a [`DATE_ONLY_MARKERS`] entry,
/// **and** it carries more date structure than a lone year — a span separator
/// (`2018 – 2021`), an open end (`2021 – Present`, `2021 – Today`) or a month
/// (`Jan 2022`).
///
/// Both halves are load-bearing, and [`looks_like_date_span`] alone is neither.
/// It is satisfied by any text carrying a year, so the word test was doing all
/// the work — and a single BARE YEAR passes the word test too. "Promoted to
/// Staff Engineer, 2022" is an ordinary line in an experience section, and
/// reading its trailing year as a date column made [`trailing_date_column`]
/// hand the sentence in front of it to the employer salvage. A year on its own
/// is a date a line MENTIONS; a column is a date the line is STRUCTURED by.
///
/// Cost, deliberately paid: an entry line whose whole date column is one bare
/// year ("Acme Corp, 2022") no longer opens a role, so its bullets continue the
/// entry above it. That is the pre-existing behaviour for every unrecognised
/// line, and it invents nothing.
///
/// The open end is read from the marker TOKEN, not from [`is_open_ended`],
/// and [`PRESENT_MARKERS`] is checked here exactly the way [`DATE_ONLY_MARKERS`]
/// beside it always was. [`is_open_ended`] requires an explicit span separator
/// (`-`, `to`, `bis`, …) because it also runs against prose; this function
/// never sees prose, because the `date_words` gate above has already rejected
/// any line carrying a word that is not a digit, a month or a marker. Routing
/// the separator-free column (`2015 Present`, `2015 Heute` — a shape
/// `export::parser::DATE_RE` accepts) through `is_open_ended` therefore
/// borrowed a restriction that protects a different caller: the column stopped
/// opening a role, its bullets rejoined the entry above, employer salvage
/// never ran, and `factual::unsupported_date_issues` — which reads date
/// context through [`trailing_date_column`] — went silent on those lines.
/// The two lists stay disjoint (asserted in the test suite); this reads both,
/// it does not merge them.
pub(super) fn is_date_only(s: &str) -> bool {
    let tokens = word_tokens(s);
    let date_words = tokens.iter().all(|t| {
        t.chars().all(|c| c.is_ascii_digit())
            || MONTH_TOKENS.contains(&t.as_str())
            || PRESENT_MARKERS.contains(&t.as_str())
            || DATE_ONLY_MARKERS.contains(&t.as_str())
    });
    if !date_words || !looks_like_date_span(s) {
        return false;
    }
    !date_spans(s).is_empty()
        || is_open_ended(s)
        || tokens.iter().any(|t| MONTH_TOKENS.contains(&t.as_str()))
        || tokens.iter().any(|t| {
            DATE_ONLY_MARKERS.contains(&t.as_str()) || PRESENT_MARKERS.contains(&t.as_str())
        })
}

/// True when one pipe/middot SEGMENT is the entry's date column: it carries a
/// YEAR.
///
/// Same test as [`looks_like_date_span`] today, kept as its own named
/// function so this call site reads as "is this segment a date column" —
/// [`is_open_ended`] no longer independently supplies a positive here without
/// a year (see its doc), which is what used to make the two diverge: a bare
/// present-tense marker with no year anywhere used to satisfy
/// `looks_like_date_span` on its own, and [`PRESENT_MARKERS`] is a list of
/// ordinary words that real employers are named after — Current
/// (current.com) and Current Health are both real, and "Aktuell" opens
/// plenty of German company names. Such a segment used to get selected as
/// the date column AND filtered out of the label segments, so the job TITLE
/// was recorded as the employer and the employer as the date span.
///
/// Deliberately NOT [`is_date_only`], which the review suggested: it is too
/// tight here, rejecting a lone year — this arm has always read
/// `Senior Engineer | Acme Corp | 2022` as an entry with a one-year column.
/// Its word test would additionally reject spellings the PARSER accepts and
/// hands us — "2018 to 2021", "2021 bis Heute", "Jan 2018 through Mar 2021"
/// all carry a word that is neither month, number nor marker — turning a
/// fixed false employer into a lost date column. (It is no longer "too
/// loose" the way it once was: `is_date_only("Current")` is false today too,
/// for the same year-required reason `is_open_ended` no longer fires on the
/// word alone — but the lone-year rejection above still makes it the wrong
/// test for this call site.)
///
/// The residual is the pre-existing one, unchanged: a label that happens to
/// carry a year ("2020 Ventures") still reads as the date column. Telling that
/// apart needs the word test this rejects.
pub(super) fn is_date_column_segment(s: &str) -> bool {
    looks_like_date_span(s)
}

/// Split `text` into `(label, dates)` when it ends in a `, <dates>` column —
/// the shape of an entry line the parser did not recognise as a `JobEntry`
/// ("Acme Payments, Berlin, 2018 - 2021").
///
/// `None` for anything else, including a line that merely mentions a year: the
/// tail must be [`is_date_only`], or an ordinary sentence ending in
/// "…, delivered in 2019" would read as an employer plus a date column.
///
/// Public because [`extract_evidence`] is no longer the only surface that has to
/// answer "does this line OPEN A ROLE?": `validate::content::split_sections`
/// refuses to promote a line to a section heading when the line below it opens
/// one, and the two must agree about what that means or a job title above an
/// employer becomes a heading on one surface and an entry label on the other.
pub fn trailing_date_column(text: &str) -> Option<(&str, &str)> {
    let (label, dates) = text.rsplit_once(',')?;
    let label = label.trim();
    (!label.is_empty() && is_date_only(dates)).then_some((label, dates.trim()))
}

/// Every 1900–2099 year in `s`, in order, deduplicated by position (not value).
///
/// Bounded to that window on purpose: a 4-digit run outside it is a quantity
/// ("processed 4500 orders"), not a date, and treating it as one is how a
/// fabricated-metric check turns into a false accusation.
pub fn years_in(s: &str) -> Vec<u32> {
    let bytes: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i - start == 4 {
            let year: u32 = bytes[start..i]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            if (1900..=2099).contains(&year) {
                out.push(year);
            }
        }
    }
    out
}
