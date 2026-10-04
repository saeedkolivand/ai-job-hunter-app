//! The date-shape predicates: open-ended spans, date columns, present-tense markers and years.

use super::*;

#[test]
fn open_ended_spans_are_recognised_in_every_spelling() {
    for open in [
        "2021 - Present",
        "2021 – Heute",
        "seit 2021",
        "since Jan 2021",
        "from 2019",
        "2021 –",
        "2021 -",
    ] {
        assert!(is_open_ended(open), "{open:?} has no end date");
    }
    for closed in [
        "2018 - 2021",
        "Jan 2018 to Mar 2021",
        "presented the roadmap",
        "knowledge sharing",
        "currently",
        "actually shipped it",
        "since the rewrite",
    ] {
        assert!(!is_open_ended(closed), "{closed:?} is not an open span");
    }
}

/// Every [`PRESENT_MARKERS`] spelling — driven off the list itself, so a
/// future addition to the list is covered automatically without anyone
/// remembering to extend this test by hand. This is the complement to
/// [`open_ended_spans_are_recognised_in_every_spelling`]'s hardcoded literals
/// above: a list-driven loop alone would not catch a marker silently dropped
/// FROM the regex construction while staying in the list (both would drift
/// together), so the hardcoded spellings stay as the independent check for
/// that.
#[test]
fn every_present_marker_opens_a_span_adjacent_to_a_year() {
    let broken: Vec<&str> = PRESENT_MARKERS
        .iter()
        .filter(|m| !is_open_ended(&format!("2021 - {m}")))
        .copied()
        .collect();
    assert!(
        broken.is_empty(),
        "not recognised adjacent to a year: {broken:?}"
    );

    // And the mirror: none of them alone, with no year anywhere, is a span —
    // the accepted-loss case this fix trades for correctness (see
    // `is_open_ended`'s doc comment).
    let bare_still_open: Vec<&str> = PRESENT_MARKERS
        .iter()
        .filter(|m| is_open_ended(m))
        .copied()
        .collect();
    assert!(
        bare_still_open.is_empty(),
        "bare marker with no year must not be open-ended: {bare_still_open:?}"
    );
}

/// A separator-free date column still opens a role — and the three prose
/// controls the whole `is_open_ended` tightening exists for stay closed.
///
/// `trailing_date_column` is the "does this line open a role?" predicate five
/// callers share (`documents::evidence`, `validate::content::{ats, factual,
/// split_sections}` and `credentials::education`). It answers through
/// [`is_date_only`], which used to reach a separator-free column
/// (`…, 2015 Present`) only via [`is_open_ended`]. When that function started
/// requiring an explicit span separator, the column stopped being a column:
/// the bullets under it silently joined the entry above, employer salvage
/// never ran, and `unsupported_date_issues` went quiet on those lines.
///
/// [`is_date_only`] can read a bare marker safely where [`is_open_ended`]
/// cannot, and the difference is structural rather than a judgement call:
/// every token on the line must already be a digit, a month or a marker, so
/// prose is rejected before any marker list is consulted. The three prose
/// controls at the bottom assert exactly that — they are the defect this PR
/// was opened for, and they must stay false however loose the column reading
/// gets.
#[test]
fn a_separator_free_date_column_opens_a_role_without_reopening_the_prose_leak() {
    for column in [
        "2015 Present",     // no separator at all — the regression
        "2015 Heute",       // …in German
        "2015 - Present",   // dash
        "2015 – Present",   // en dash
        "(2015 - Present)", // parenthesised
        "2015 - 2018",      // closed span
        "Jan 2022",         // month + year
    ] {
        let line = format!("Acme Payments, Berlin, {column}");
        assert_eq!(
            trailing_date_column(&line),
            Some(("Acme Payments, Berlin", column)),
            "{column:?} must read as a date column"
        );
    }

    // Negative controls — a line that merely MENTIONS a year is not a column,
    // and a lone year is the documented, deliberate non-answer. Without these
    // the loop above passes for a `trailing_date_column` that accepts any
    // comma tail carrying a digit.
    assert_eq!(
        trailing_date_column("Owned the ledger rewrite, delivered in 2019"),
        None,
        "prose ending in a year is not a date column"
    );
    assert_eq!(
        trailing_date_column("Acme Payments, Berlin, 2022"),
        None,
        "a lone year is a date the line MENTIONS, not one it is structured by"
    );

    // The prose leak this PR fixed, re-asserted at its own function: none of
    // these is an open-ended span, however many markers or years they carry.
    for prose in [
        "Reduced actual costs by 20% in 2023",
        "Led the current platform migration",
        "Shipped the ongoing rewrite",
    ] {
        assert!(
            !is_open_ended(prose),
            "{prose:?} is ordinary prose, not a date span"
        );
    }
}

#[test]
fn contains_word_respects_boundaries() {
    assert!(contains_word("this is vital work", "vital"));
    assert!(!contains_word("we revitalized the pipeline", "vital"));
    assert!(contains_word("not just faster, but cheaper", "not just"));
    assert!(!contains_word("", "vital"));
    assert!(!contains_word("anything", ""));
}

#[test]
fn years_in_ignores_non_year_digit_runs() {
    assert_eq!(years_in("2021 - Present"), vec![2021]);
    assert_eq!(years_in("Jan 2018 to Mar 2021"), vec![2018, 2021]);
    assert!(
        years_in("processed 4500 orders").is_empty(),
        "a quantity outside 1900–2099 is not a year"
    );
    assert!(
        years_in("+49 30 1234567").is_empty(),
        "a phone number's digit runs are not years"
    );
}

/// The unit half of the same rule, both directions: every shape that IS a date
/// column, and the bare year that is not.
#[test]
fn a_date_column_needs_more_than_a_year_in_it() {
    for column in [
        "2018 - 2021",
        "2018 – 2021",
        "05/2018 – 07/2021",
        "Jan 2018 - Mar 2021",
        "2021 - Present",
        "2021 - Heute",
        "2021 –",
        "Jan 2022",
    ] {
        assert!(is_date_only(column), "{column:?} is a date column");
    }
    for prose in ["2022", "delivered in 2019", "40 warehouse sites", ""] {
        assert!(!is_date_only(prose), "{prose:?} is not a date column");
    }
}

/// Every "today"/"currently"-family spelling in [`DATE_ONLY_MARKERS`] — English
/// `Today`/`Currently`, Spanish `Actualidad`/`Actualmente`/`Hoy`, Portuguese
/// `Hoje`/`Atualmente`, French `Aujourd'hui`/`Actuellement`, Italian `Oggi`,
/// and the `Present`-adjacent `Presente`/`Attualmente`/`Derzeit`/`Vandaag` —
/// opens the date column it appears in.
///
/// Collects every failure instead of asserting inside the loop: a bare
/// `assert!` per iteration reports only the FIRST broken spelling and hides
/// every other one behind it, so a future regression that drops or typos a
/// second marker would read identically to "everything fine" for this test.
/// This one call names every spelling that stopped working.
///
/// This test is deliberately NOT paired with a prose loop the way an earlier
/// version was — every one of those "prose" rows only passed because
/// `word_tokens` splits the sentence into words the `date_words.all()` gate
/// (a digit, a month or a marker) already rejects before any marker is even
/// consulted, so the row proved the tokenizer works on ordinary sentences,
/// not that [`DATE_ONLY_MARKERS`] is safe. [`a_date_column_needs_more_than_a_year_in_it`]
/// already covers that mechanism; duplicating it here under a name that
/// implies it guards this list would be misleading about what failed if it
/// ever went red.
#[test]
fn date_only_markers_open_every_added_spelling() {
    let columns = [
        "2020 - Today",
        "2015 - Actualidad",
        "2020 - Actualmente",
        "2019 - Aujourd'hui",
        "2021 - Oggi",
        "2020 - Presente",
        "2020 - Currently",
        "2020 - Hoje",
        "2020 - Atualmente",
        "2020 - Hoy",
        "2020 - Actuellement",
        "2020 - Attualmente",
        "2020 - Derzeit",
        "2020 - Vandaag",
    ];
    let not_recognised: Vec<&str> = columns
        .into_iter()
        .filter(|column| !is_date_only(column))
        .collect();
    assert!(
        not_recognised.is_empty(),
        "not read as date columns: {not_recognised:?}"
    );
}

/// The safety argument for [`DATE_ONLY_MARKERS`] depends entirely on it never
/// sharing a spelling with [`PRESENT_MARKERS`] — [`is_open_ended`] and
/// `validate::content::factual::employment::unsupported_date_issues` both match
/// [`PRESENT_MARKERS`] against arbitrary free text, so a spelling living in
/// both lists would turn ordinary prose carrying that word into a false date
/// context. That invariant lived only in the const's doc comment; this makes
/// keeping it cost one assertion instead of a careful re-read before the next
/// spelling is added.
#[test]
fn date_only_markers_never_overlap_present_markers() {
    let overlap: Vec<&str> = DATE_ONLY_MARKERS
        .iter()
        .filter(|m| PRESENT_MARKERS.contains(m))
        .copied()
        .collect();
    assert!(
        overlap.is_empty(),
        "spellings in both lists — is_open_ended would fire on them as bare prose: {overlap:?}"
    );
}
