//! The accepted misses of the language check, and the evidence primitives that bound it.

use super::{support::*, *};

/// A review found that gating the evidence requirement on
/// `CURATED_FUNCTION_WORDS` membership (seven languages) instead of the
/// actual Latin SCRIPT (nine — Turkish and Vietnamese are Latin-script too)
/// let a confident `tr`/`vi` whatlang guess skip corroboration entirely and
/// raise a Critical with ZERO evidence. `needs_distinctive_evidence` now
/// gates on the nine-language script set; since this crate curates no
/// `tr`/`vi` vocabulary, a genuine `tr`/`vi` mismatch now goes quiet instead —
/// an ACCEPTED miss, the same shape as every other uncurated-language miss
/// this module already documents (see `regime_4`), not a new kind of gap.
#[test]
fn turkish_text_needs_evidence_too_and_goes_quiet_without_a_curated_list() {
    let tr_text = "Ödeme sistemleri ve konteyner platformlarında sekiz yıllık deneyime sahip \
        bir backend mühendisiyim. Ödeme sistemleri ve konteyner platformlarında sekiz yıllık \
        deneyime sahip bir backend mühendisiyim.";
    assert!(
        significant_chars(tr_text) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(tr_text),
        Some("tr"),
        "premise: must confidently read as Turkish, or this test proves nothing about \
         the script gate specifically"
    );
    assert!(
        super::language::needs_distinctive_evidence("tr"),
        "Turkish is Latin-script and must still need corroboration — it is the exact \
         language this fix closes a zero-evidence hole for"
    );
    assert!(
        !is_language_mismatch(tr_text, "en"),
        "an accepted miss: no curated tr vocabulary means no evidence, so this stays \
         quiet rather than firing on zero corroboration"
    );
}

/// **Advisory MEDIUM (confirmation review), the close-relative-target
/// silent band.** [`MIN_DISTINCTIVE_HITS`] is one absolute floor for every
/// language pair, but a genuine short paragraph in one Romance language
/// carries far fewer PAIRWISE-distinctive words against a close relative
/// than against a distant one — es/pt/fr/it/nl share most short function
/// words with each other and only diverge on the handful pairwise pruning
/// keeps. Measured: the SAME 120-150 character genuine two-sentence
/// paragraph that clears the floor and fires cleanly against `"en"`
/// (evidence 6) sits at evidence 1-4 — under the floor, silent — against a
/// linguistically close target. This repo ships es/pt/it/nl locale profiles
/// (see [`crate::locale`]), so a genuinely wrong-language document
/// targeting one of THESE instead of English is a real, supported scenario,
/// not a hypothetical one — the same accepted-cost shape as the DACH miss
/// and the tr/vi miss above, pinned here so a future change to
/// [`MIN_DISTINCTIVE_HITS`] cannot quietly start believing the floor has no
/// blind spot among the languages this crate actually curates. Not closed
/// here: lowering the floor to catch these would reopen the false positives
/// [`distinctive_evidence_confirms_requires_the_shipped_floor`]'s mutation
/// check already found at floor 4 — the same "closing one language's miss
/// reopens another's bug" trade every other suppressor in this module hit.
#[test]
fn a_short_paragraph_against_a_close_relative_target_is_an_accepted_miss() {
    use super::language::pairwise_evidence_count;

    let es_two_sentences = "Trabaje como ingeniero de software durante dos anos en Madrid, \
        con experiencia solida en Python y en sistemas distribuidos de gran escala y alta \
        disponibilidad.";
    assert!(
        significant_chars(es_two_sentences) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor, or the silence below proves nothing \
         about the evidence gate specifically"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(es_two_sentences),
        Some("es"),
        "premise: must confidently read as Spanish"
    );
    assert_eq!(
        pairwise_evidence_count(es_two_sentences, "es", "fr"),
        1,
        "pinned measurement: a genuine Spanish paragraph carries almost no \
         French-exclusive-looking evidence against French specifically"
    );
    assert!(
        !is_language_mismatch(es_two_sentences, "fr"),
        "accepted miss: Spanish text targeting French stays quiet — evidence 1 is well \
         under the floor"
    );
    assert!(
        is_language_mismatch(es_two_sentences, "en"),
        "premise: the SAME text fires against a distant target, proving the silence above \
         is about the LANGUAGE PAIR, not a defect in the fixture"
    );

    let pt_two_sentences = "Trabalhei como engenheiro de software em Lisboa durante dois \
        anos, com experiência sólida em Python e em sistemas distribuídos de grande escala.";
    assert!(
        significant_chars(pt_two_sentences) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(pt_two_sentences),
        Some("pt"),
        "premise: must confidently read as Portuguese"
    );
    for other in ["es", "fr", "nl"] {
        assert_eq!(
            pairwise_evidence_count(pt_two_sentences, "pt", other),
            4,
            "[pt vs {other}] pinned measurement: one hit short of the floor"
        );
        assert!(
            !is_language_mismatch(pt_two_sentences, other),
            "[pt vs {other}] accepted miss: evidence 4 is one hit under the floor"
        );
    }
    assert!(
        is_language_mismatch(pt_two_sentences, "en"),
        "premise: the SAME Portuguese text fires against a distant target"
    );
}

/// **Advisory MEDIUM (confirmation review), the connector-sparse-register
/// accepted miss.** [`MIN_DISTINCTIVE_HITS`] cannot separate "an English
/// document that merely names a foreign institution" (correctly quiet) from
/// "a genuinely foreign document written in a register too sparse in
/// closed-class connectors to evidence itself" (incorrectly quiet) — both
/// land in the SAME 0-4 evidence band, because a pure count has no way to
/// tell them apart. Standard German CV register drops the article and the
/// finite-verb clause a sentence would otherwise need ("Zahlungsplattform
/// aufgebaut", not "Ich habe die Zahlungsplattform aufgebaut"), so even a
/// WHOLE, genuinely German résumé written this way carries almost no
/// evidence: measured here at 3, on 695 significant characters — two hits
/// short of the floor of 5.
///
/// **Not recoverable by lowering the floor.** 3 is the exact value
/// [`MIN_DISTINCTIVE_HITS`]'s own corpus table already relies on staying
/// quiet for a genuine false positive one row up (a German institution name
/// inside an EDUCATION section of an otherwise-English document) — a floor
/// of 3 would reopen that false positive at the same moment it closes this
/// false negative. This is the THIRD accepted miss this module documents,
/// after the uncurated `tr`/`vi` miss
/// (`turkish_text_needs_evidence_too_and_goes_quiet_without_a_curated_list`)
/// and the close-relative-target Romance-language miss
/// (`a_short_paragraph_against_a_close_relative_target_is_an_accepted_miss`
/// above) — and, same as those two, a DESIGN limit, not a calibration
/// error: recovering it needs a signal this module does not have
/// (participle/compound morphology, or a second corroborating witness), not
/// a threshold change. Left to a future change rather than folded into
/// whichever fix happens to be touching this file; pinned here so that
/// future change has something concrete to turn green — the fixture below,
/// currently silent, firing `content.language_mismatch`.
///
/// Both `is_language_mismatch` AND [`document_language_mismatch`] — the
/// SAME function `pipeline::resume::stages::draft` gates its
/// draft-language retry on — are asserted quiet, so this pins that
/// regeneration does not fire for this shape either, not just that the
/// validator does not.
#[test]
fn a_terse_participle_register_german_resume_is_an_accepted_miss() {
    use super::language::pairwise_evidence_count;

    let de_terse = "Jane Doe\n\
        jane.doe@example.com | +49 30 1234567 | github.com/janedoe\n\n\
        ZUSAMMENFASSUNG\n\n\
        Acht Jahre Zahlungsverkehr, Aufbau Container-Plattformen, Senior Backend Engineering.\n\n\
        BERUFSERFAHRUNG\n\n\
        Senior Backend Engineer, Acme Payments, 2021 - heute\n\
        - Zahlungsplattform aufgebaut, Wartezeit gesenkt: 480ms auf 90ms\n\
        - Migration Kubernetes-Cluster geleitet, zwölftausend Anfragen pro Sekunde\n\
        - Wiederholungsplaner neu entwickelt in Rust, fehlgeschlagene Zahlungen reduziert\n\n\
        Backend Developer, Globex Logistics, 2018 - 2021\n\
        - Abrechnungsschnittstelle entwickelt in Python, PostgreSQL, vierzig Lagerstandorte\n\
        - Flotte migriert zu AWS, Terraform als Bereitstellungswerkzeug\n\n\
        KENNTNISSE\n\n\
        Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis\n\n\
        AUSBILDUNG\n\n\
        BSc Informatik, TU Berlin, 2014 - 2018\n";

    assert!(
        significant_chars(de_terse) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor, or the silence below proves nothing \
         about the evidence gate specifically"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(de_terse),
        Some("de"),
        "premise: must confidently read as German, or this is not the shape this test claims"
    );
    let evidence = pairwise_evidence_count(de_terse, "de", "en");
    assert_eq!(
        evidence, 3,
        "pinned measurement: a genuinely German résumé in the connector-sparse participle \
         register carries almost no closed-class evidence even at whole-résumé length"
    );
    assert!(
        evidence < 5,
        "premise: the miss below only proves what this test claims if evidence is genuinely \
         under MIN_DISTINCTIVE_HITS's shipped floor (5), not accidentally at or over it"
    );
    assert!(
        !is_language_mismatch(de_terse, "en"),
        "accepted miss: a genuinely German whole résumé, in the connector-sparse register \
         real German CVs use, stays quiet — this is the gap a future recall improvement \
         needs to close (participle/compound morphology, or a second witness), and it must \
         do so WITHOUT reopening the EDUCATION-section false positive that sits at the same \
         evidence count"
    );
    assert!(
        !document_language_mismatch(de_terse, EN_SOURCE, EN_JOB_AD, "en"),
        "the same accepted miss through the public entry point pipeline::resume::stages::draft \
         gates its language-retry on — regeneration does not fire for this shape either"
    );
    let report = en_resume(de_terse, &en_requirements());
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// `pairwise_evidence_count`'s exclusion rules, isolated from any whole-
/// document fixture: a shared word (in BOTH lists) never counts; a bare
/// single-letter token never counts even when it IS a genuine word in
/// `lang`'s list; a word `lang` shares with some THIRD language `other` has
/// never heard of still counts (this is the whole point of pairwise over a
/// global pool); "zu" (a genuine German preposition, 2 characters, absent
/// from `FUNCTION_WORDS_EN`) DOES count — 2-character tokens are not
/// universally excluded, only the single-letter floor and a genuine target
/// collision remove a word.
#[test]
fn pairwise_evidence_count_excludes_shared_and_single_letter_tokens() {
    use super::language::pairwise_evidence_count;

    // "und" is German-only (not in FUNCTION_WORDS_EN) and 3 characters — counts.
    assert_eq!(pairwise_evidence_count("und und und", "de", "en"), 3);
    // "in" is shared by German AND English — never counts either way.
    assert_eq!(pairwise_evidence_count("in in in", "de", "en"), 0);
    // "zu" is a genuine German preposition, only 2 characters, and NOT shared
    // with English — counts. (Confirms the length floor is 2, not 3: a
    // blanket 3-character floor was tried and measured to also exclude
    // Italian's own "il"/"la"/"di" — see MIN_DISTINCTIVE_TOKEN_CHARS's doc.)
    assert_eq!(pairwise_evidence_count("zu zu zu", "de", "en"), 3);
    // "y" ("and", Spanish) is a genuine word but only ONE character — excluded
    // by the single-letter floor even though it appears nowhere in
    // FUNCTION_WORDS_EN.
    assert_eq!(pairwise_evidence_count("y y y", "es", "en"), 0);
    // Pairwise, not global: "con" collides between Spanish and Italian, but
    // that collision is irrelevant when comparing Spanish against ENGLISH,
    // which has never heard of "con" at all.
    assert!(pairwise_evidence_count("con con con", "es", "en") > 0);
    // An uncurated language (no list at all) always returns zero.
    assert_eq!(pairwise_evidence_count("und und und", "tr", "en"), 0);
}

/// German capitalises every noun, so a nominal-register connector
/// ("Aufbau und Betrieb der Zahlungsplattform") sits between two capitalised
/// words exactly as often as a proper noun's own connector does — a
/// title-case-sandwich exclusion tried in `pairwise_evidence_count` could not
/// tell the two apart and deleted German's evidence wholesale (see the
/// module doc's `MIN_DISTINCTIVE_HITS` history). Pinned here directly: this
/// exact shape must still count.
#[test]
fn a_connector_between_two_title_case_words_still_counts() {
    use super::language::pairwise_evidence_count;

    assert_eq!(
        pairwise_evidence_count("Technik und Wirtschaft", "de", "en"),
        1,
        "\"und\" between two Capitalised words is ordinary German nominal register, \
         not a signal to exclude it"
    );
}

/// `distinctive_evidence_confirms`'s absolute floor, isolated: `found`'s
/// pairwise evidence must clear [`MIN_DISTINCTIVE_HITS`] (below it, a match
/// is noise); a non-Latin-script `found` skips the requirement entirely
/// regardless of evidence, since `needs_distinctive_evidence` never asks a
/// script whatlang already reads reliably to corroborate itself.
///
/// Mutation check (performed at the WHOLE-SUITE level, not just this
/// isolated pair — both classes really do respond to the one constant):
///
/// - Floor moved to 4 (one under the shipped 5) — RAN the full
///   `validate::content` suite: `a_german_institution_name_inside_an_english_certifications_section_never_fires`
///   (a QUIET fixture, evidence 4) went red — a false positive reopened.
/// - Floor moved to 7 (one over) — RAN the full suite again:
///   `a_drifted_volunteer_section_warns_rather_than_blocks` (a FIRE fixture,
///   evidence 6) went red — a false negative reopened.
/// - Restored to 5; full suite green again.
///
/// This IS the property a shape-based exclusion cannot offer: one number,
/// and both failure directions are visibly, mechanically tied to it.
#[test]
fn distinctive_evidence_confirms_requires_the_shipped_floor() {
    use super::language::distinctive_evidence_confirms;

    // Below the floor (4 hits): must not confirm.
    assert!(!distinctive_evidence_confirms(
        "und und und und",
        "de",
        "en"
    ));
    // At the floor (5 hits): confirms.
    assert!(distinctive_evidence_confirms(
        "und und und und und",
        "de",
        "en"
    ));
    // A non-Latin script skips the requirement outright: zero evidence, but
    // whatlang's script read is already reliable there.
    assert!(distinctive_evidence_confirms(
        "zero evidence text here",
        "zh",
        "en"
    ));
}

/// `needs_distinctive_evidence` gates on the nine-language Latin-SCRIPT set,
/// not on which seven have a curated vocabulary — the distinction BLOCKING 2
/// (tr/vi) hinges on.
#[test]
fn needs_distinctive_evidence_is_the_nine_language_latin_script_set() {
    for lang in ["en", "de", "fr", "es", "it", "nl", "pt", "tr", "vi"] {
        assert!(
            super::language::needs_distinctive_evidence(lang),
            "{lang} is Latin-script and must need corroboration"
        );
    }
    for lang in ["zh", "ja", "ko", "ar", "he", "hi", "bn", "th", "uk", "ru"] {
        assert!(
            !super::language::needs_distinctive_evidence(lang),
            "{lang} reads a non-Latin script; whatlang's script read is already reliable \
             there without corroboration"
        );
    }
}

/// The floor BLOCKING 3 asked for in place of an exact-count assertion — a
/// language's own pairwise vocabulary against English must clear a healthy
/// minimum, so a future edit that silently thins a list back down (the exact
/// way the Spanish regression shipped unnoticed) fails loudly here instead of
/// only showing up as a missed Critical three layers away. Chosen well below
/// every language's current raw list size (`en` 70, `de` 83, `fr` 68, `es`
/// 61, `it` 65, `nl` 58, `pt` 58) so ordinary curation additions/removals do
/// not make this brittle.
#[test]
fn every_curated_language_has_a_healthy_pairwise_floor_against_english() {
    const FLOOR: usize = 40;
    for lang in ["de", "fr", "es", "it", "nl", "pt"] {
        let words = super::language::function_words_for(lang);
        let against_en: usize = words
            .iter()
            .filter(|w| !super::language::function_words_for("en").contains(w))
            .count();
        assert!(
            against_en >= FLOOR,
            "{lang}'s vocabulary against English has only {against_en} pairwise-distinct \
             entries, under the {FLOOR} floor — this is exactly how the Spanish regression \
             (30 survivors after global pruning) shipped unnoticed; investigate before \
             lowering this floor"
        );
    }
}

/// Hygiene guard for the class of bug a review found by inspection: `"zijn"`
/// listed TWICE in `FUNCTION_WORDS_NL` (possessive and copula) — harmless
/// under the current HashSet-based [`pairwise_evidence_count`] (a duplicate
/// collapses on `.collect()`), but a silent authoring mistake nonetheless,
/// and the SAME sweep found a second instance (`"a"` and `"se"`, each twice,
/// in `FUNCTION_WORDS_PT`) the review did not catch by inspection. Both are
/// fixed; this test is what keeps a third one from shipping the same way.
#[test]
fn no_curated_language_list_has_an_internal_duplicate() {
    for lang in ["en", "de", "fr", "es", "it", "nl", "pt"] {
        let words = super::language::function_words_for(lang);
        let unique: std::collections::HashSet<&&str> = words.iter().collect();
        assert_eq!(
            unique.len(),
            words.len(),
            "{lang}'s function-word list has an internal duplicate — {} entries, {} unique; \
             {words:?}",
            words.len(),
            unique.len()
        );
    }
}
