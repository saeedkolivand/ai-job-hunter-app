//! `voice.restated_point`: one point said twice in other words fires; shared key terms,
//! posting vocabulary, short letters, thin-stopword languages and language mismatches stay
//! quiet, and both thresholds are pinned from each side.

use super::{support::*, *};

const AD: &str = "Acme Payments seeks a Senior Rust Engineer in Berlin to own billing \
                  infrastructure and reliability.";
const DE_AD: &str = "Die Musterbank Frankfurt sucht einen Senior Softwareentwickler für die \
                     Zahlungsplattform.";

/// Six unrelated sentences so a pair under test sits in a letter long enough to be checked.
const PADDING: [&str; 6] = [
    "Photography and long distance cycling fill most of my weekends.",
    "I volunteer teaching evening mathematics classes at a community library.",
    "Moving across three countries taught me patience with unfamiliar processes.",
    "My manager would describe me as calm during outages.",
    "Your posting mentions reliability work that I would enjoy.",
    "I would welcome a conversation about the team.",
];
const DE_PADDING: [&str; 6] = [
    "Am Wochenende fahre ich lange Strecken mit dem Rennrad.",
    "Abends unterrichte ich ehrenamtlich Mathematik in einer Bibliothek.",
    "Drei Umzüge in andere Länder haben mir Geduld beigebracht.",
    "Mein Vorgesetzter beschreibt mich als ruhig bei Störungen.",
    "Über ein Gespräch mit Ihrem Team würde ich mich freuen.",
    "Vielen Dank für Ihre Zeit.",
];

/// A letter: greeting, `pair` as the first two sentences, padding, sign-off.
fn letter(lang: &str, pair: [&str; 2], padding: &[&str; 6], ad: &str) -> ContentReport {
    let source = if lang == "de" { DE_SOURCE } else { EN_SOURCE };
    let body = format!(
        "Dear Hiring Manager,\n\n{} {} {}\n\nSincerely,\nJane Doe",
        pair[0],
        pair[1],
        padding.join(" ")
    );
    letter_in(lang, &body, source, ad)
}

fn en(pair: [&str; 2]) -> ContentReport {
    letter("en", pair, &PADDING, AD)
}

const MENTOR: [&str; 2] = [
    "I mentored four junior engineers through their first on-call rotations last year.",
    "Last year I coached four junior engineers through their first on-call rotations.",
];

#[test]
fn restated_sentence_fires_with_the_pair_as_evidence() {
    let report = en(MENTOR);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(evidence[0].contains("I mentored four junior engineers"));
    assert!(evidence[0].contains("Last year I coached four junior engineers"));
}

#[test]
fn evidence_quotes_the_closest_pair_not_the_first() {
    let looser = [
        "Nightly reconciliation batches caught ledger drift between finance systems.",
        "Nightly reconciliation batches caught ledger drift between treasury platforms.",
    ];
    let padding = [
        MENTOR[0], MENTOR[1], PADDING[2], PADDING[3], PADDING[4], PADDING[5],
    ];
    let report = letter("en", looser, &padding, AD);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(
        evidence[0].contains("I mentored four junior engineers"),
        "{evidence:?}"
    );
    assert!(!evidence[0].contains("Nightly"), "{evidence:?}");
}

#[test]
fn a_repeated_key_term_is_not_a_restatement() {
    let report = en([
        "Last year I moved our billing services onto Kubernetes.",
        "Kubernetes also let us cut the cost of idle staging clusters by a third.",
    ]);
    let third = "Teaching the support rotation to debug Kubernetes pods took a full quarter.";
    let padding = [
        third, PADDING[1], PADDING[2], PADDING[3], PADDING[4], PADDING[5],
    ];
    let with_third = letter(
        "en",
        [
            "Last year I moved our billing services onto Kubernetes.",
            "Kubernetes also let us cut the cost of idle staging clusters by a third.",
        ],
        &padding,
        AD,
    );
    silent(&report, VOICE_RESTATED_POINT);
    silent(&with_third, VOICE_RESTATED_POINT);
}

/// Three sentences about the same subject, each adding something new: the shared vocabulary
/// is the letter's topic, so no pair of them is a restatement (a pair alone would be, see
/// `just_above_the_overlap_threshold_fires`).
#[test]
fn a_term_set_used_in_three_sentences_is_the_letters_subject() {
    let report = letter(
        "en",
        [
            "Nightly reconciliation batches caught ledger drift between finance systems.",
            "Nightly reconciliation batches caught ledger drift between treasury platforms.",
        ],
        &[
            "Nightly reconciliation batches caught ledger drift between audit pipelines.",
            PADDING[1],
            PADDING[2],
            PADDING[3],
            PADDING[4],
            PADDING[5],
        ],
        AD,
    );
    silent(&report, VOICE_RESTATED_POINT);
}

#[test]
fn posting_vocabulary_shared_by_opening_and_closing_is_not_a_restatement() {
    let open =
        "I am excited to apply for the Senior Rust Engineer role at Acme Payments in Berlin.";
    let close = "My background fits the Senior Rust Engineer role at Acme Payments in Berlin.";
    silent(&en([open, close]), VOICE_RESTATED_POINT);
    // With the posting unknown the same two sentences ARE mostly one vocabulary, so the
    // subtraction (not the sentence shape) is what keeps this quiet.
    let unknown = letter(
        "en",
        [open, close],
        &PADDING,
        "Unrelated text about gardening.",
    );
    fired(&unknown, VOICE_RESTATED_POINT);
}

#[test]
fn german_bookends_naming_the_role_stay_quiet() {
    let report = letter(
        "de",
        [
            "Ich bewerbe mich auf die Stelle als Senior Softwareentwickler bei der Musterbank Frankfurt.",
            "Meine Erfahrung passt gut zur Stelle als Senior Softwareentwickler bei der Musterbank Frankfurt.",
        ],
        &DE_PADDING,
        DE_AD,
    );
    silent(&report, VOICE_RESTATED_POINT);
}

#[test]
fn german_unrelated_sentences_sharing_a_verb_form_stay_quiet() {
    let report = letter(
        "de",
        [
            "Im letzten Jahr habe ich die Migration der Zahlungsdienste geleitet.",
            "Davor habe ich die Schulung der Bereitschaftsteams geleitet.",
        ],
        &DE_PADDING,
        DE_AD,
    );
    silent(&report, VOICE_RESTATED_POINT);
}

#[test]
fn german_restatement_fires() {
    let report = letter(
        "de",
        [
            "Im letzten Jahr habe ich vier junge Kollegen durch ihre ersten Bereitschaftsdienste begleitet.",
            "Vier junge Kollegen habe ich im letzten Jahr durch ihre ersten Bereitschaftsdienste begleitet.",
        ],
        &DE_PADDING,
        DE_AD,
    );
    fired(&report, VOICE_RESTATED_POINT);
}

/// Italian has no curated function-word list (its stopwords are job-ad filler that
/// leaves "della", "nella", "alla" in), so even a plain restatement goes quiet.
#[test]
fn italian_goes_quiet() {
    let it = [
        "Nell'ultimo anno ho guidato quattro colleghi junior durante i primi turni di reperibilità.",
        "Durante i primi turni di reperibilità ho guidato quattro colleghi junior nell'ultimo anno.",
    ];
    silent(&letter("it", it, &PADDING, AD), VOICE_RESTATED_POINT);
}

#[test]
fn an_uncurated_language_goes_quiet() {
    silent(&letter("fr", MENTOR, &PADDING, AD), VOICE_RESTATED_POINT);
}

/// The mismatch gate on its own: an `en` letter written in German fires no restatement
/// even though `en` passes the language-list gate.
#[test]
fn a_letter_in_the_wrong_language_goes_quiet() {
    let body = format!(
        "Sehr geehrte Damen und Herren,\n\nIm letzten Jahr habe ich vier junge Kollegen durch \
         ihre ersten Bereitschaftsdienste begleitet. Vier junge Kollegen habe ich im letzten \
         Jahr durch ihre ersten Bereitschaftsdienste begleitet. {}\n\nMit freundlichen Grüßen\nJane Doe",
        DE_PADDING.join(" ")
    );
    silent(&letter_in("en", &body, EN_SOURCE, AD), VOICE_RESTATED_POINT);
}

#[test]
fn a_short_letter_is_not_checked() {
    let short = format!("{} {}", MENTOR[0], MENTOR[1]);
    silent(
        &letter_in("en", &short, EN_SOURCE, AD),
        VOICE_RESTATED_POINT,
    );
}

#[test]
fn greeting_and_signoff_lines_never_pair() {
    let block = "Dear Hiring Manager,\n\nSincerely,\nJane Doe\n\n";
    silent(
        &letter_in("en", &block.repeat(4), EN_SOURCE, AD),
        VOICE_RESTATED_POINT,
    );
}

/// Jaccard 0.5 (6 shared of 12): different points sharing a lot of vocabulary stay quiet.
#[test]
fn just_below_the_overlap_threshold_stays_quiet() {
    silent(
        &en([
            "Nightly reconciliation batches caught ledger drift between finance systems.",
            "Nightly reconciliation batches caught ledger errors between treasury platforms.",
        ]),
        VOICE_RESTATED_POINT,
    );
}

/// Jaccard 0.64 (7 shared of 11): just above the threshold.
#[test]
fn just_above_the_overlap_threshold_fires() {
    fired(
        &en([
            "Nightly reconciliation batches caught ledger drift between finance systems.",
            "Nightly reconciliation batches caught ledger drift between treasury platforms.",
        ]),
        VOICE_RESTATED_POINT,
    );
}

/// Five content words each is enough; four is not, even at overlap 1.0.
#[test]
fn the_minimum_content_word_boundary_is_four_versus_five() {
    fired(
        &en([
            "The nightly reconciliation batches caught drift.",
            "Drift was caught by nightly reconciliation batches.",
        ]),
        VOICE_RESTATED_POINT,
    );
    silent(
        &en([
            "The nightly reconciliation batches caught it.",
            "It was caught by nightly reconciliation batches.",
        ]),
        VOICE_RESTATED_POINT,
    );
}
