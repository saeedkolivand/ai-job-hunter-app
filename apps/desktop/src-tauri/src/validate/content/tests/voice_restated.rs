//! `voice.restated_point`: one point said twice in other words fires; parallel sentences
//! with different facts, posting vocabulary, short or oversized letters, thin-stopword
//! languages and language mismatches stay quiet, and every threshold is pinned from both sides.

use super::{support::*, *};

const AD: &str = "Acme Payments seeks a Senior Rust Engineer in Berlin to own billing \
                  infrastructure and reliability.";
const DE_AD: &str = "Die Musterbank Frankfurt sucht einen Senior Softwareentwickler für die \
                     Zahlungsplattform.";

/// Unrelated sentences so a pair under test sits in a letter long enough to be checked.
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

const MENTOR: [&str; 2] = [
    "I mentored four junior engineers through their first on-call rotations last year.",
    "Last year I coached four junior engineers through their first on-call rotations.",
];
/// Content words: A has 8, B has 9, 7 shared, so Jaccard is exactly 7/10.
const AT_THRESHOLD: [&str; 2] = [
    "Nightly reconciliation batches caught ledger drift in finance systems.",
    "Nightly reconciliation batches caught ledger drift in finance platforms and audits.",
];
/// A has 8, B has 7, 6 shared: Jaccard 6/9, just under the threshold.
const JUST_BELOW: [&str; 2] = [
    "Nightly reconciliation batches caught ledger drift in finance systems.",
    "Nightly reconciliation batches caught ledger drift in treasury.",
];
const WEEKLY: [&str; 2] = [
    "Weekly deployment dashboards flagged latency regressions in checkout services.",
    "Weekly deployment dashboards flagged latency regressions in checkout flows and alerts.",
];

fn body(lang: &str, sentences: &[&str], ad: &str) -> ContentReport {
    let source = if lang == "de" { DE_SOURCE } else { EN_SOURCE };
    letter_in(lang, &sentences.join(" "), source, ad)
}

/// `pair` followed by the six padding sentences, in a framed letter.
fn framed(lang: &str, pair: [&str; 2], padding: &[&str; 6], ad: &str) -> ContentReport {
    let source = if lang == "de" { DE_SOURCE } else { EN_SOURCE };
    let text = format!(
        "Dear Hiring Manager,\n\n{} {} {}\n\nSincerely,\nJane Doe",
        pair[0],
        pair[1],
        padding.join(" ")
    );
    letter_in(lang, &text, source, ad)
}

fn en(pair: [&str; 2]) -> ContentReport {
    framed("en", pair, &PADDING, AD)
}

#[test]
fn restated_sentence_fires_with_the_pair_as_evidence() {
    let report = en(MENTOR);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(evidence[0].contains("I mentored four junior engineers"));
    assert!(evidence[0].contains("Last year I coached four junior engineers"));
    assert!(evidence[0].ends_with("(1 pair(s))"), "{evidence:?}");
}

/// A point restated three times in the same words is the clearest case, and must
/// not be hidden by the term being "everywhere".
#[test]
fn a_point_restated_three_times_fires() {
    let text = [
        "Nightly reconciliation batches caught ledger drift in finance systems.",
        "The nightly reconciliation batches caught ledger drift in finance systems.",
        "Nightly reconciliation batches caught ledger drift in the finance systems.",
    ];
    let sentences: Vec<&str> = text.iter().chain(PADDING.iter()).copied().collect();
    let report = body("en", &sentences, AD);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(evidence[0].ends_with("(3 pair(s))"), "{evidence:?}");
}

/// The closest pair sits in the MIDDLE, so a first-wins or last-wins pick both fail.
#[test]
fn evidence_quotes_the_closest_pair_and_counts_all_pairs() {
    let sentences = [
        AT_THRESHOLD[0],
        AT_THRESHOLD[1],
        MENTOR[0],
        MENTOR[1],
        WEEKLY[0],
        WEEKLY[1],
        PADDING[2],
        PADDING[3],
    ];
    let report = body("en", &sentences, AD);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(
        evidence[0].contains("I mentored four junior engineers"),
        "{evidence:?}"
    );
    assert!(evidence[0].ends_with("(3 pair(s))"), "{evidence:?}");
}

#[test]
fn long_sentences_are_truncated_so_the_pair_count_survives() {
    let filler = "wonderful ".repeat(40);
    let a =
        format!("Nightly reconciliation batches caught ledger drift in finance systems {filler}.");
    let b = format!(
        "Nightly reconciliation batches caught ledger drift in finance platforms {filler}."
    );
    let sentences: Vec<&str> = [a.as_str(), b.as_str()]
        .into_iter()
        .chain(PADDING.iter().copied())
        .collect();
    let report = body("en", &sentences, AD);
    let evidence = fired_evidence(&report, VOICE_RESTATED_POINT);
    assert!(evidence[0].ends_with("(1 pair(s))"), "{evidence:?}");
}

/// Two parallel sentences that state DIFFERENT facts in one frame (Jaccard about 0.6).
#[test]
fn parallel_sentences_with_different_facts_stay_quiet() {
    silent(
        &en([
            "During the first quarter I led the migration of our payment services onto the new cluster.",
            "During the second quarter I led the migration of our reporting services onto the new cluster.",
        ]),
        VOICE_RESTATED_POINT,
    );
    silent(
        &en([
            "Nightly reconciliation batches caught ledger drift between finance systems.",
            "Nightly reconciliation batches caught ledger drift between treasury platforms.",
        ]),
        VOICE_RESTATED_POINT,
    );
}

#[test]
fn posting_vocabulary_shared_by_opening_and_closing_is_not_a_restatement() {
    let open = "I am applying for the Senior Rust Engineer role at Acme Payments in Berlin.";
    let close = "This fits the Senior Rust Engineer role at Acme Payments in Berlin.";
    silent(&en([open, close]), VOICE_RESTATED_POINT);
    silent(
        &en([
            "I am excited to apply for the Senior Rust Engineer role at Acme Payments.",
            "My background makes me a strong fit for the Senior Rust Engineer role at Acme Payments.",
        ]),
        VOICE_RESTATED_POINT,
    );
    // With the posting unknown the same two sentences ARE mostly one vocabulary, so the
    // subtraction (not the sentence shape) is what keeps this quiet.
    let unknown = framed(
        "en",
        [open, close],
        &PADDING,
        "Unrelated text about gardening.",
    );
    fired(&unknown, VOICE_RESTATED_POINT);
}

#[test]
fn german_bookends_naming_the_role_stay_quiet() {
    let report = framed(
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

/// Ordinary German words that survive tokenization (habe, mich, nicht, letzten) inflate
/// overlap; two different points built on them must still land under the threshold.
#[test]
fn german_sentences_sharing_ordinary_words_stay_quiet() {
    let report = framed(
        "de",
        [
            "Im letzten Jahr habe ich mich nicht auf die Migration der Zahlungsdienste konzentriert.",
            "Im letzten Jahr habe ich mich nicht auf die Schulung der Bereitschaftsteams konzentriert.",
        ],
        &DE_PADDING,
        DE_AD,
    );
    silent(&report, VOICE_RESTATED_POINT);
}

#[test]
fn german_restatement_fires() {
    let report = framed(
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
    silent(&framed("it", it, &PADDING, AD), VOICE_RESTATED_POINT);
}

#[test]
fn an_uncurated_language_goes_quiet() {
    silent(&framed("fr", MENTOR, &PADDING, AD), VOICE_RESTATED_POINT);
}

/// The mismatch gate on its own: an `en` letter written in German fires no restatement
/// even though `en` passes the language-list gate.
#[test]
fn a_letter_in_the_wrong_language_goes_quiet() {
    let text = format!(
        "Sehr geehrte Damen und Herren,\n\nIm letzten Jahr habe ich vier junge Kollegen durch \
         ihre ersten Bereitschaftsdienste begleitet. Vier junge Kollegen habe ich im letzten \
         Jahr durch ihre ersten Bereitschaftsdienste begleitet. {}\n\nMit freundlichen Grüßen\nJane Doe",
        DE_PADDING.join(" ")
    );
    silent(&letter_in("en", &text, EN_SOURCE, AD), VOICE_RESTATED_POINT);
}

/// One greeting and one sign-off in an otherwise ordinary letter: only the
/// minimum-content-word gate keeps those two lines out of every pair.
#[test]
fn greeting_and_signoff_lines_never_pair() {
    let text = format!(
        "Dear Hiring Manager,\n\n{}\n\nSincerely,\nJane Doe",
        PADDING.join(" ")
    );
    silent(&letter_in("en", &text, EN_SOURCE, AD), VOICE_RESTATED_POINT);
}

/// A sentence-count boundary: 8 sentences are checked, 7 are not.
#[test]
fn the_minimum_sentence_boundary_is_seven_versus_eight() {
    let letter = |n: usize| {
        let mut s = MENTOR.to_vec();
        s.extend(std::iter::repeat_n("Thank you.", n - 2));
        body("en", &s, AD)
    };
    fired(&letter(8), VOICE_RESTATED_POINT);
    silent(&letter(7), VOICE_RESTATED_POINT);
}

/// The pairwise scan is bounded: a document past the cap is not a letter and goes quiet.
#[test]
fn an_oversized_document_goes_quiet() {
    let letter = |n: usize| {
        let mut s = MENTOR.to_vec();
        s.extend(std::iter::repeat_n("Thank you.", n - 2));
        body("en", &s, AD)
    };
    fired(&letter(80), VOICE_RESTATED_POINT);
    silent(&letter(81), VOICE_RESTATED_POINT);
}

/// Jaccard 6/9 (about 0.67): different points sharing most of their vocabulary stay quiet.
#[test]
fn just_below_the_overlap_threshold_stays_quiet() {
    silent(&en(JUST_BELOW), VOICE_RESTATED_POINT);
}

/// Jaccard exactly 7/10: the threshold is inclusive.
#[test]
fn at_the_overlap_threshold_fires() {
    fired(&en(AT_THRESHOLD), VOICE_RESTATED_POINT);
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
