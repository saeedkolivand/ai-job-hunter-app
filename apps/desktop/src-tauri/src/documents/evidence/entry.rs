//! Entry-line splitting: which segment of a résumé entry line is the employer, which is the
//! title, and which token identifies a specific company at all.
//!
//! Moved verbatim out of [`super`] (which was 29 lines under the R8 hard cap), and
//! split again so the date-shape family lives in [`super::dates`].
//!
//! Every public name here is re-exported from [`super`], so `documents::evidence::split_entry`
//! and friends keep resolving exactly as before; no caller moved with the code.

use crate::export::types::ParsedLine;

use super::dates::{is_date_column_segment, word_tokens};

/// Legal-form suffixes: they name a corporate structure, never an employer.
/// "GmbH" appearing in a document proves nothing about which GmbH.
pub const LEGAL_FORMS: &[&str] = &[
    "gmbh",
    "corp",
    "corporation",
    "limited",
    "incorporated",
    "holding",
    "group",
    "company",
    "inc",
    "ltd",
    "llc",
    "plc",
    "ag",
    "kg",
    "se",
    "bv",
    "nv",
    "sa",
    "srl",
    "spa",
];

/// Country/region/city tokens an employer's legal name — or an entry line's
/// location column — carries. Same argument as [`LEGAL_FORMS`]: "Deutschland"
/// or "Berlin" identifies no particular employer, so neither may be the SOLE
/// reason `validate::content` decides an entry survived, and a comma-tail made
/// of nothing but these is a location rather than a company (see
/// [`split_entry`]).
pub const GEOGRAPHY_TOKENS: &[&str] = &[
    "deutschland",
    "germany",
    "österreich",
    "austria",
    "schweiz",
    "switzerland",
    "europe",
    "europa",
    "emea",
    "apac",
    "dach",
    "international",
    "global",
    "worldwide",
    "berlin",
    "münchen",
    "munich",
    "hamburg",
    "wien",
    "zürich",
];

/// The tokens of `company` that name a SPECIFIC employer: alphanumeric runs of
/// `min_chars`+ characters, with legal forms and geography removed.
///
/// One helper, two questions, deliberately — they differ only in that
/// threshold. `validate::content::factual` asks "could anything in the
/// generated document evidence this employer?" and accepts two characters
/// ("SAP" is a real name); `validate::content::consistency` asks "are these two
/// entries the same employer?" and needs a distinctive four. Keeping a second
/// copy of the exclusion lists beside either question is exactly how one of
/// them silently kept "Berlin" and started matching two unrelated Berlin
/// employers to each other.
pub fn identity_tokens(company: &str, min_chars: usize) -> Vec<String> {
    word_tokens(company)
        .into_iter()
        .filter(|t| t.chars().count() >= min_chars)
        .filter(|t| !LEGAL_FORMS.contains(&t.as_str()))
        .filter(|t| !GEOGRAPHY_TOKENS.contains(&t.as_str()))
        .collect()
}

/// Whether `text` **ends** with a corporate legal form.
///
/// Positional on purpose. A legal form is a SUFFIX — "Nordwind Systeme GmbH",
/// "Acme Inc", "Globex Ltd" — while [`LEGAL_FORMS`] necessarily also holds
/// `group`, `company` and `holding`, which are ordinary English title words.
/// Testing every token made the head of "Group Product Manager, Acme Payments"
/// look like a company name, so [`split_two_space_label`]'s company-first rule
/// fired and recorded the employer as "Group Product Manager" — discarding both
/// the real company and the title.
///
/// The words stay in [`LEGAL_FORMS`] because [`identity_tokens`] needs them for
/// a different question ("does this token name a SPECIFIC employer?", where
/// `group` is worthless wherever it sits). Only this split heuristic needs the
/// position.
fn ends_with_legal_form(text: &str) -> bool {
    word_tokens(text)
        .last()
        .is_some_and(|t| LEGAL_FORMS.contains(&t.as_str()))
}

/// Whether `s` is made of NOTHING but [`GEOGRAPHY_TOKENS`] — a location column
/// ("Berlin", "Munich, Germany"), never an employer.
///
/// Shared by both entry-splitting arms in [`split_entry`] on purpose. The
/// two-space arm was hardened against reading the location as the company; the
/// pipe/middot arm was not, so "Acme Corp | Berlin | 2021 – Present" recorded
/// the CITY as the employer and the employer as the job title — the same defect
/// class, one heuristic away. A company whose whole name is a listed
/// geography word ("Berlin") is dropped rather than kept: the same tradeoff
/// [`split_two_space_label`] already accepts, and inventing nothing beats
/// naming a city as an employer.
fn is_location_only(s: &str) -> bool {
    let tokens = word_tokens(s);
    !tokens.is_empty()
        && tokens
            .iter()
            .all(|t| GEOGRAPHY_TOKENS.contains(&t.as_str()))
}

/// Split the two-space form's label into `(company, title)`.
///
/// This form is the extracted-PDF shape, and its comma tail is a LOCATION far
/// more often than a company — the title usually sits on the line BELOW, which
/// is what [`extract_evidence`]'s `LineKind::JobTitle` arm exists to pick up.
/// Reusing the parenthesized form's title-first "split at the last comma" rule
/// here named the CITY as the employer, and `validate::content` then went
/// looking for that city in the generated document and raised a
/// `factual.dropped_role` Critical when a tailored résumé left it out.
///
/// Only two signals are trusted, both from lists this module already owns, and
/// anything they cannot resolve keeps the title-first reading:
///
/// * a tail made of nothing but [`GEOGRAPHY_TOKENS`] is a location, however
///   many segments it runs to (`Globex Logistics, Munich, Germany`);
/// * failing that, a TRAILING legal form in the HEAD and none at the end of the
///   tail (`Nordwind Systeme GmbH, Ingolstadt`) — an unlisted city cannot be
///   told from a company by shape, so that is the only evidence left. Trailing,
///   not anywhere: see [`ends_with_legal_form`].
///
/// A label that is NOTHING but geography resolves to nothing at all. The peel
/// loop strips location-only comma TAILS one at a time, so what it leaves can
/// itself be a location ("Berlin, Germany" → "Berlin"), and the comma-less arm
/// below returns whatever it is handed as the company — which named a CITY as
/// the employer. Both other arms of [`split_entry`] already refuse to do that;
/// this one guessed. Unattributed beats invented, exactly as [`is_location_only`]
/// says.
pub(super) fn split_two_space_label(label: &str) -> (String, String) {
    let mut label = label.trim();
    while let Some((head, tail)) = label.rsplit_once(',') {
        if !is_location_only(tail) {
            break;
        }
        label = head.trim();
    }
    if is_location_only(label) {
        return (String::new(), String::new());
    }
    match label.rsplit_once(',') {
        Some((head, tail)) if ends_with_legal_form(head) && !ends_with_legal_form(tail) => {
            (head.trim().to_string(), String::new())
        }
        Some((title, company)) => (company.trim().to_string(), title.trim().to_string()),
        None => (label.to_string(), String::new()),
    }
}

/// How many words a comma-less entry label may run to and still read as an
/// employer's NAME. Four covers "Nordwind Systeme Digital Solutions"; past that
/// a label is a sentence far more often than a company.
const MAX_COMPANY_LABEL_WORDS: usize = 4;

/// Whether `label` reads as a NAME rather than as a sentence: at most
/// [`MAX_COMPANY_LABEL_WORDS`] words, none of which STARTS lowercase.
///
/// Capitalization is the discriminator because it is the one signal that
/// separates the two shapes in both curated languages without a vocabulary.
/// English writes an employer in title case ("Acme Payments") and a sentence
/// with lowercase function words ("Led **the** platform rewrite"); German
/// capitalizes its nouns but never its articles, prepositions or verbs
/// ("Beförderung **zum** Staff Engineer"), so a prose fragment in either
/// language almost always carries a lowercase word. Digits and symbols are
/// neutral, so "3M Deutschland" and "Johnson & Johnson" both pass.
///
/// The two errors it can still make, both stated rather than hidden:
///
/// * a title-cased JOB TITLE with no company on the line ("Leitende
///   Entwicklerin, Jan 2019 - Dec 2021") is recorded as the employer;
/// * an employer written all-lowercase ("adidas, Jan 2019 - Dec 2021") stays
///   unattributed.
///
/// The second is the safe direction and the first is bounded: this value never
/// reaches a `factual.dropped_role` comparison, which reads only lines the
/// parser recognised as a `JobEntry`.
fn looks_like_a_company_name(label: &str) -> bool {
    let words: Vec<&str> = label.split_whitespace().collect();
    !words.is_empty()
        && words.len() <= MAX_COMPANY_LABEL_WORDS
        && words.iter().all(|w| {
            w.chars()
                .find(|c| c.is_alphanumeric())
                .is_none_or(|c| !c.is_lowercase())
        })
}

/// Salvage `(company, title)` from an entry line the PARSER did not recognise
/// — [`extract_evidence`]'s role-opening arm, where the label was cut out of
/// running text instead of handed over by `export::parser`. `None` means
/// nothing could be resolved, and the caller must open an UNATTRIBUTED role
/// rather than name one.
///
/// ## The comma-less arm, and why it may trust its input again
///
/// [`split_two_space_label`]'s comma-less arm reads the whole label as the
/// company. That is correct where IT is called from — the parser has already
/// peeled the date column off a template-rendered entry, so whatever remains is
/// the employer by construction — and the same construction now holds here:
/// [`trailing_date_column`] only reaches this function after [`is_date_only`]
/// has confirmed a real date COLUMN, so the label is everything in front of one.
/// "Acme Payments, Jan 2019 - Dec 2021" is an ordinary entry line whose label IS
/// the employer, and refusing to name it (R7's behaviour) cost every such entry
/// its attribution while keeping the name as bullet text.
///
/// What the gate does NOT decide is whether the label is a NAME:
/// "Led the platform rewrite, Jan 2019 - Dec 2021" passes it too. That is
/// [`looks_like_a_company_name`]'s job, and it is why this is not simply a call
/// to [`split_two_space_label`] — the promotion note and the sentence must
/// still open an unattributed role, exactly as R7 pinned them.
///
/// Text is never deleted to buy either behaviour: when this returns `None` the
/// caller keeps the label as the new role's first bullet.
pub(super) fn salvage_entry_label(label: &str) -> Option<(String, String)> {
    if !label.contains(',') {
        let label = label.trim();
        return looks_like_a_company_name(label).then(|| (label.to_string(), String::new()));
    }
    let (company, title) = split_two_space_label(label);
    (!company.is_empty() || !title.is_empty()).then_some((company, title))
}

/// Split an entry line into `(company, title, dates)`.
///
/// HEURISTIC, and deliberately a conservative one — a wrong split must never
/// invent a company the résumé does not name. Three shapes, matching the three
/// `LineKind::JobEntry` forms `export::parser` produces:
///
/// 1. Two-space form (`Acme Corp    2021 – Present`): the parser already split
///    the date into `right_text`, so `text` is the entry label, split by
///    [`split_two_space_label`]'s company-first rule.
/// 2. Pipe/middot form (`Senior Engineer | Acme Corp | 2021 – Present`): the
///    date segment ([`is_date_column_segment`], which needs a year — a bare
///    present-tense marker is an employer's NAME at least as often as a date)
///    is the span, and of the two remaining segments the first is the title and
///    the second the company — UNLESS the first carries a legal-form suffix and
///    the second does not, which is the one shape that says otherwise. See
///    "Which segment is the company" below. Location-only segments are dropped
///    first ([`is_location_only`]) — an entry line carries a location column at
///    least as often as a title, and positional reading alone recorded
///    "Acme Corp | Berlin | 2021 – Present" as the company "Berlin".
/// 3. Parenthesized form (`Senior Engineer, Acme Corp (Jan 2021 – Mar 2023)`):
///    the parenthesized tail is the span, and the label splits at the LAST
///    comma.
///
/// A label with no separator becomes the company with an empty title — a
/// following `LineKind::JobTitle` line fills that in.
///
/// ## Which segment of the pipe form is the company
///
/// "Title first" was justified as "the order every template in this repo
/// renders". That is true of what this app GENERATES and false of what a user
/// UPLOADS: "Acme Corp | Senior Engineer | 2021 – Present" is an ordinary
/// company-first entry line, and reading it positionally recorded the employer
/// as "Senior Engineer" — which merges every entry sharing that title into one
/// employer for `validate::content::consistency`, and hands the generation
/// prompt an [`super::EvidenceRole`] whose company is a job title.
///
/// The discriminator is the cheapest signal that separates the two orders
/// without a vocabulary: a trailing legal form ([`ends_with_legal_form`], the
/// same positional test [`split_two_space_label`] already arbitrates with).
/// When the FIRST segment ends in one and the second does not, the line is
/// company-first; otherwise the title-first reading stands, so every pinned
/// case ("Senior Engineer | Acme Corp", "IT-Beraterin | IBM Deutschland GmbH")
/// is untouched.
///
/// **The conceded residual, stated plainly:** a company-first line whose
/// employer carries no legal form ("Acme Payments | Senior Engineer | 2021 –
/// Present") still reads title-first. Nothing on that line distinguishes the
/// two orders — both segments are title-cased noun phrases — and the
/// alternatives are a closed title vocabulary (which drifts, and which no
/// locale list here could keep honest) or a cross-entry positional vote (which
/// needs the other entries and is wrong on a one-entry résumé). A wrong guess
/// here names an employer the résumé never had, so the rule stays at the one
/// signal that cannot be argued with.
///
/// Public so `validate::content` decides "did this employer survive?" against
/// the same company string the evidence extractor derived. Splitting the label
/// twice, differently, is how a dropped-role check ends up comparing a job
/// TITLE against the output and concluding nothing was lost.
pub fn split_entry(line: &ParsedLine) -> (String, String, String) {
    let label_and_dates = |label: &str, dates: &str| {
        let (title, company) = match label.rsplit_once(',') {
            Some((t, c)) => (t.trim().to_string(), c.trim().to_string()),
            None => (String::new(), label.trim().to_string()),
        };
        (company, title, dates.trim().to_string())
    };

    if let Some(dates) = line.right_text.as_deref() {
        let (company, title) = split_two_space_label(&line.text);
        return (company, title, dates.trim().to_string());
    }

    let separators = ['|', '·', '•'];
    if line.text.contains(separators) {
        let segments: Vec<&str> = line.text.split(separators).map(str::trim).collect();
        let dates = segments
            .iter()
            .find(|s| is_date_column_segment(s))
            .map(|s| s.to_string())
            .unwrap_or_default();
        let rest: Vec<&str> = segments
            .iter()
            .copied()
            .filter(|s| !s.is_empty() && !is_date_column_segment(s) && !is_location_only(s))
            .collect();
        return match rest.as_slice() {
            [] => (String::new(), String::new(), dates),
            [only] => (only.to_string(), String::new(), dates),
            // Company-first: a legal-form SUFFIX on the leading segment and
            // none on the next one. See "Which segment of the pipe form is the
            // company" above.
            [company, title, ..]
                if ends_with_legal_form(company) && !ends_with_legal_form(title) =>
            {
                (company.to_string(), title.to_string(), dates)
            }
            [title, company, ..] => (company.to_string(), title.to_string(), dates),
        };
    }

    match line.text.rsplit_once('(') {
        Some((label, tail)) => label_and_dates(label, tail.trim_end_matches(')')),
        None => label_and_dates(&line.text, ""),
    }
}
