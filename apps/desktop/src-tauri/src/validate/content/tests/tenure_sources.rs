//! The evidence side of the tenure check: how far a source's dates reach, and the number
//! words and decades it may state a tenure in.

use super::credential_corpus::*;
use super::{support::*, *};

/// The tenure check reads the system clock — once, to close an open-ended
/// "2019 – Present" — and this pins what happens when the clock cannot be
/// trusted.
///
/// A clock reading EARLIER than a year the documents themselves name is wrong,
/// and using it would shrink the allowance into an accusation. The span
/// evidence is dropped instead. Both halves are asserted, because a test that
/// only shows silence cannot tell "the guard worked" from "nothing ever fires":
/// the same document without the future year reports the same claim.
#[test]
fn a_year_the_clock_has_not_reached_drops_the_span_evidence_rather_than_shrinking_it() {
    let source = "Jane Doe

EXPERIENCE

         Backend Developer | Globex Logistics | 2019 - Present
         - Built the billing API in Python and PostgreSQL
";
    let with_future_year = format!(
        "{source}- Signed the platform support contract through 2099
"
    );
    let generated = summary_claiming("Backend engineer with 20 years of experience in payments.");

    assert!(
        codes(&report_against(&generated, source)).contains(&FACTUAL_INFLATED_EXPERIENCE),
        "control: against a 2019-Present source, twenty years is an inflation"
    );
    silent(
        &report_against(&generated, &with_future_year),
        FACTUAL_INFLATED_EXPERIENCE,
    );
}

/// A2a — the separator-free date column (`2016 Present`, no dash at all).
/// `export::parser::DATE_RE` itself accepts this shape as a job entry's date
/// range with no separator required between the year and the marker, so a
/// résumé written this way is not a contrived fixture. Before
/// `credentials::tenure::names_a_present_marker` existed, `source_is_ongoing`
/// missed it entirely: the span closed at the role's own single stated year
/// (2016-2016), and a truthful "8 years of experience" against a role that
/// started in 2016 read as inflated. The control proves the widened
/// allowance is not just "nothing ever fires": an implausible claim still
/// does.
#[test]
fn a_separator_free_ongoing_date_column_widens_the_allowance() {
    let source = "Jane Doe\n\n\
         EXPERIENCE\n\n\
         Backend Developer | Globex Logistics | 2016 Present\n\
         - Built the billing API in Python and PostgreSQL\n";

    let truthful = summary_claiming("Backend engineer with 8 years of experience in payments.");
    silent(
        &report_against(&truthful, source),
        FACTUAL_INFLATED_EXPERIENCE,
    );

    let implausible = summary_claiming("Backend engineer with 45 years of experience in payments.");
    assert!(
        codes(&report_against(&implausible, source)).contains(&FACTUAL_INFLATED_EXPERIENCE),
        "control: an implausible claim must still fire even with the widened allowance"
    );
}

/// A2a separator sweep — the shape of the SEPARATOR between a role's start
/// year and its present-tense marker must not decide whether the role is
/// still open.
///
/// `source_is_ongoing` is read here through the value it exists to produce,
/// `career_span_years`: an open role's span runs to the reference year, a
/// closed one stops at the last year the source names. Measured regression:
/// when the marker branch required an explicit span separator
/// (`-`, `to`, `bis`, …), every pipe/middot/comma/parenthesised spelling
/// below collapsed from an eleven-year span to a zero-year one, and a
/// truthful "11 years of experience" became a false
/// `factual.inflated_experience` Critical.
///
/// Not a contrived set: `export::parser::DATE_RE`, this codebase's own
/// definition of a job entry's date range, allows up to 30 ARBITRARY
/// characters between the year and the marker, so every spelling here is one
/// the parser itself already reads as a date range.
///
/// The closed-history control is what stops this passing for the wrong
/// reason. Without it, an implementation that simply called every line
/// ongoing would satisfy every other row.
#[test]
fn any_separator_between_a_year_and_a_present_marker_keeps_the_role_open() {
    let source_with = |dates: &str| {
        format!(
            "Jane Doe\n\nEXPERIENCE\n\n\
             Senior Engineer | Globex Logistics | {dates}\n\
             - Built the billing API in Python and PostgreSQL\n"
        )
    };

    for dates in [
        "2015 - Present", // dash — never broke
        "2015 | Present", // pipe
        "2015 | Aktuell", // pipe, German marker
        "2015 · Present", // middot
        "2015, Present",  // comma
        "2015 (ongoing)", // parenthesised, no separator at all
        "2015 Present",   // whitespace only, no separator at all
    ] {
        assert_eq!(
            credentials::career_span_years(&source_with(dates), Some(2026)),
            Some(11),
            "{dates:?}: an open role must run to the reference year"
        );
    }

    // Negative control: a genuinely CLOSED history still closes at its own
    // last year. A test without this row passes for an implementation that
    // answers "ongoing" unconditionally.
    assert_eq!(
        credentials::career_span_years(&source_with("2005 - 2015"), Some(2026)),
        Some(10),
        "a closed history must stop at the last year the source names"
    );
}

/// The allowance, anchored to an ABSOLUTE year rather than to today.
///
/// `career_span_years` takes its reference year as an argument precisely so
/// this can be pinned: `EN_SOURCE_FOUR_YEARS` runs 2016-2020 and has no open
/// role, so the span is 4 whatever the calendar says, and the allowance is 5.
/// The end-to-end boundary test below then exercises the same numbers through
/// `validate_content`, where a closed history makes the result
/// calendar-independent too.
#[test]
fn a_closed_history_spans_from_its_earliest_year_to_its_latest() {
    assert_eq!(
        credentials::career_span_years(EN_SOURCE_FOUR_YEARS, Some(2026)),
        Some(4),
        "2016 to 2020, and no open role for today to close"
    );
    assert_eq!(
        credentials::supported_years(EN_SOURCE_FOUR_YEARS, Some(2026)),
        Some(5),
        "4 dated years plus exactly one of slack"
    );
    // An OPEN history reaches today instead — same source, one role reopened.
    let ongoing = EN_SOURCE_FOUR_YEARS.replace("2018 - 2020", "2018 - Present");
    assert_eq!(
        credentials::career_span_years(&ongoing, Some(2026)),
        Some(10),
        "2016 to the reference year"
    );
}

/// A present-tense spelling `PRESENT_MARKERS` does not carry must still read as
/// ONGOING, or the span closes at the role's own start year and a truthful
/// résumé reports itself as an exaggeration.
///
/// Measured on a fully truthful Spanish document: `2015 - Actualidad` produced
/// `factual.inflated_experience` as the only Critical. `Aujourd'hui` and
/// `Today` behave identically, and none of the three is in the lexicon — which
/// is why the test is structural (a separator followed by a non-year) rather
/// than a fourth word added to a list.
///
/// Mutation check: make `source_is_ongoing` return `false` and every arm below
/// goes red.
#[test]
fn an_unknown_present_marker_still_reads_as_an_open_span() {
    for marker in ["Actualidad", "Aujourd'hui", "Today", "Heden", "Attualmente"] {
        let source = format!(
            "Ana García\n\nEXPERIENCIA\n\n\
             Ingeniera de Backend | Acme Pagos | 2015 - {marker}\n\
             - Construyó la plataforma de liquidación\n\n\
             Desarrolladora | Globex | 2011 - 2015\n\
             - Mantuvo la API de facturación\n"
        );
        assert_eq!(
            credentials::career_span_years(&source, Some(2026)),
            Some(15),
            "{marker}: 2011 to the reference year, because the role has not ended"
        );
    }
    // The negative half: a history that really is closed does NOT reach today.
    let closed = "Ana García\n\nEXPERIENCIA\n\n\
         Ingeniera de Backend | Acme Pagos | 2015 - 2019\n\
         - Construyó la plataforma de liquidación\n";
    assert_eq!(
        credentials::career_span_years(closed, Some(2026)),
        Some(4),
        "every span in this document names its end year"
    );
}

/// A source that states a tenure this file cannot READ states an UNKNOWN
/// tenure, and unknown is not zero.
///
/// The number words now cover every language the pipeline writes, but a word
/// list can always be missing a word — "several years", "over a decade", a
/// spelling nobody listed. When the quantifier cannot be read the whole check
/// goes quiet, because the side that goes blind here is the side that SPARES.
///
/// Mutation check: delete the `states_an_unreadable_tenure` guard and the first
/// arm goes red.
#[test]
fn a_source_tenure_this_file_cannot_read_silences_the_check() {
    let unreadable = format!(
        "Jane Doe\n\nSUMMARY\n\nBackend engineer with several years of experience.\n\
         {EN_SOURCE_FOUR_YEARS}"
    );
    let claim = summary_claiming("Backend engineer with 12 years of experience in payments.");
    silent(
        &report_against(&claim, &unreadable),
        FACTUAL_INFLATED_EXPERIENCE,
    );
    // The control: the same claim against the same dates, minus the unreadable
    // sentence, is reported.
    assert!(
        codes(&report_against(&claim, EN_SOURCE_FOUR_YEARS)).contains(&FACTUAL_INFLATED_EXPERIENCE),
        "without the unreadable sentence this source supports five years, not twelve"
    );
}

/// A tenure spelled out in a language whose number words the table used to
/// miss. The comparison is on a NUMBER, but only if the SPARING evidence can be
/// read — an en/de-only table read `quinze années d'expérience` as a source
/// that states nothing and turned a faithful `15 années` into a Critical.
#[test]
fn a_tenure_spelled_out_in_any_supported_language_spares_its_own_restatement() {
    for (word, unit, digits) in [
        ("quinze", "années", "15 années"),
        ("quince", "años", "15 años"),
        ("quindici", "anni", "15 anni"),
        ("vijftien", "jaar", "15 jaar"),
        ("quinze", "anos", "15 anos"),
    ] {
        let source =
            format!("Jane Doe\n\nSUMMARY\n\n{word} {unit} d'experience.\n{EN_SOURCE_FOUR_YEARS}");
        assert_eq!(
            credentials::stated_years(&source),
            Some(15),
            "{word} {unit} states fifteen years"
        );
        let generated = summary_claiming(&format!("{digits} of experience in payments."));
        silent(
            &report_against(&generated, &source),
            FACTUAL_INFLATED_EXPERIENCE,
        );
    }
}

/// The number-word table, checked against a HAND-WRITTEN list rather than
/// against itself.
///
/// A test that loops over `SPELLED_NUMBERS` proves only that the table agrees
/// with the table; it cannot catch an omission, which is the failure mode this
/// class of table actually has. Spanish `quince`, Italian `sette` and
/// Portuguese `sete` were all missing from the first cut and were found by
/// exactly this shape.
///
/// Seven and fifteen, in all seven languages the pipeline writes: a tenure long
/// enough to be worth claiming, and a single digit, in each.
#[test]
fn the_number_word_table_reads_every_language_the_pipeline_writes() {
    let cases: &[(&str, u32)] = &[
        ("seven years", 7),
        ("fifteen years", 15),
        ("sieben Jahre", 7),
        ("fünfzehn Jahre", 15),
        ("sept ans", 7),
        ("quinze années", 15),
        ("siete años", 7),
        ("quince años", 15),
        ("sette anni", 7),
        ("quindici anni", 15),
        ("zeven jaar", 7),
        ("vijftien jaar", 15),
        ("sete anos", 7),
        ("quinze anos", 15),
    ];
    for (phrase, expected) in cases {
        assert_eq!(
            credentials::stated_years(&format!("A résumé line saying {phrase} of work.")),
            Some(*expected),
            "{phrase} must read as {expected}"
        );
    }
}

/// `$X per year` is close to the most common quantified-impact phrasing on a
/// résumé, and the unreadable-tenure guard used to read every one of these as
/// "the source states a tenure I cannot measure" — silencing
/// `factual.inflated_experience` for the whole document.
///
/// An off switch and a fix are indistinguishable to a precision-only
/// measurement: both report zero false positives. This asserts the check is
/// still AWAKE.
///
/// Mutation check: drop the `is_tenure_context` scoping from
/// `states_an_unreadable_tenure` and every arm goes red.
#[test]
fn an_impact_figure_measured_per_year_does_not_silence_the_tenure_check() {
    for bullet in [
        "- Cut cloud spend by 1.2M USD per year",
        "- Reported year over year growth of 40% to the board",
        "- Ran the fiscal year close for two entities",
        "- Mentored two interns last year",
    ] {
        let source = format!(
            "Jane Doe\n\nEXPERIENCE\n\n\
             Backend Engineer | Acme Payments | 2019 - 2021\n{bullet}\n"
        );
        assert_eq!(
            credentials::supported_years(&source, Some(2026)),
            Some(3),
            "{bullet}: the source still supports a measurable tenure"
        );
        assert!(
            codes(&report_against(
                &summary_claiming("Backend engineer with 20 years of experience."),
                &source
            ))
            .contains(&FACTUAL_INFLATED_EXPERIENCE),
            "{bullet}: an inflated claim must still be reported"
        );
    }
    // The negative half: a tenure the file genuinely cannot read still silences.
    let unreadable =
        "Jane Doe\n\nSUMMARY\n\nBackend engineer with several years of experience.\n\n\
         EXPERIENCE\n\nBackend Engineer | Acme Payments | 2019 - 2021\n- Built the platform\n";
    assert_eq!(credentials::supported_years(unreadable, Some(2026)), None);
}

/// A tenure stated in DECADES. There is no year-word to anchor on, so neither
/// the number table nor the unreadable-quantifier guard saw it, and a truthful
/// restatement earned a Critical.
///
/// Read as UNKNOWN rather than mapped to ten: "over a decade" is anywhere from
/// ten years to nineteen, and picking a number would invent evidence on the
/// sparing side.
///
/// Mutation check: delete the `DECADE_RE` branch and the first arm goes red.
#[test]
fn a_tenure_stated_in_decades_silences_the_check() {
    for phrase in [
        "over a decade of experience",
        "mehr als ein Jahrzehnt Erfahrung",
        "plus d'une décennie d'expérience",
        "más de una década de experiencia",
    ] {
        let source = format!(
            "Jane Doe\n\nSUMMARY\n\nBackend engineer with {phrase} in payments.\n\n\
             EXPERIENCE\n\nBackend Engineer | Acme Payments | 2019 - 2021\n- Built the platform\n"
        );
        assert_eq!(
            credentials::supported_years(&source, Some(2026)),
            None,
            "{phrase}: an unmeasurable tenure, not an absent one"
        );
    }
    // The control: the same source without the decade sentence is measurable,
    // so the silence above is the decade's doing.
    let measurable = "Jane Doe\n\nSUMMARY\n\nBackend engineer in payments.\n\n\
         EXPERIENCE\n\nBackend Engineer | Acme Payments | 2019 - 2021\n- Built the platform\n";
    assert_eq!(
        credentials::supported_years(measurable, Some(2026)),
        Some(3)
    );
}

/// `Seit 03/2016` — a date column opened with a NUMERIC month, which is what a
/// German, French or Spanish column usually carries.
///
/// `documents::evidence::is_open_ended` wants a year within one optional word
/// of the opener, so `Seit März 2016` is open and `Seit 03/2016` is not. The
/// current role then read as closed at its own start year and a truthful ten
/// years became a Critical.
///
/// Mutation check: delete the `SPAN_OPENER_RE` arm of `source_is_ongoing` and
/// the first three go red.
#[test]
fn a_numeric_month_after_an_opener_still_reads_as_an_open_span() {
    for opener in ["Seit", "Depuis", "Since", "Desde"] {
        let source = format!(
            "Jana Mustermann\n\nBERUFSERFAHRUNG\n\n\
             Senior Backend Engineer | Acme Payments | {opener} 03/2016\n\
             - Die Abrechnungsplattform gebaut\n"
        );
        assert_eq!(
            credentials::career_span_years(&source, Some(2026)),
            Some(10),
            "{opener} 03/2016 opens a span that has not ended"
        );
    }
    // The negative half: no opener, no year to close it with, still closed.
    let closed = "Jana Mustermann\n\nBERUFSERFAHRUNG\n\n\
         Senior Backend Engineer | Acme Payments | 03/2016 - 12/2019\n\
         - Die Abrechnungsplattform gebaut\n";
    assert_eq!(
        credentials::career_span_years(closed, Some(2026)),
        Some(3),
        "a column that names both ends is closed"
    );
}

/// [`SPELLED_NUMBERS`] repeats two spellings across language blocks
/// (`quatorze`: French AND Portuguese; `tres`: Spanish AND Portuguese) — both
/// map to the same value in each case, so the repeat is harmless for what
/// `YEARS_RE` matches, but only once the word list is actually deduplicated.
///
/// Mutation check: sort by `Reverse(len)` alone before deduping (the bug
/// `spelled_number_words` replaced) and this goes red — the two repeats above
/// are no longer adjacent once grouped by LENGTH instead of by VALUE, so
/// `Vec::dedup` (consecutive-only) misses them.
#[test]
fn the_spelled_number_vocabulary_has_no_duplicate_spelling() {
    let words = credentials::spelled_number_words();
    let mut sorted = words.clone();
    sorted.sort_unstable();
    let mut deduped = sorted.clone();
    deduped.dedup();
    assert_eq!(
        sorted, deduped,
        "a spelling repeats in the alternation: {words:?}"
    );
}
