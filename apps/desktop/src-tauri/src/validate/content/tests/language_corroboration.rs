//! `content.language_mismatch` at document scope: the cross-language regimes, and which
//! witnesses may vouch for a target language.

use super::{support::*, *};

/// Short text is where `whatlang` guesses, so the Critical language check must
/// go quiet below [`MIN_CHARS_FOR_LANGUAGE_CHECK`] — a two-line draft is not
/// evidence of the wrong language.
#[test]
fn short_documents_never_raise_a_language_critical() {
    let short = "Guten Tag.";
    assert!(
        short.chars().filter(|c| !c.is_whitespace()).count() < MIN_CHARS_FOR_LANGUAGE_CHECK,
        "fixture must be under the floor, or this test proves nothing"
    );
    assert!(!is_language_mismatch(short, "en"));
    // And just past the floor, the same language really is flagged.
    let long_german = "Sehr geehrte Damen und Herren, hiermit bewerbe ich mich auf die \
                       ausgeschriebene Stelle als Backend-Entwicklerin in Ihrem Unternehmen \
                       und freue mich sehr über eine Rückmeldung von Ihnen.";
    assert!(
        long_german.chars().filter(|c| !c.is_whitespace()).count() >= MIN_CHARS_FOR_LANGUAGE_CHECK
    );
    assert!(is_language_mismatch(long_german, "en"));
}

/// H3 (premise restated for `target_is_corroborated`) — the ORIGINAL claim was
/// "the detector reads the source the same wrong way it reads the output, so
/// the detector is the unreliable party." That framing no longer fits: BOTH
/// fixtures here really are German (`detected_language` reads them
/// confidently, correctly, as `"de"` — this is not a misdetection). The
/// accurate reason this stays quiet is narrower and still real: NEITHER
/// witness (job ad, source résumé) confidently reads as `"en"`, so `"en"`
/// itself is not corroborated as a credible target, and the accusation has
/// no reliable premise to stand on. This is the documented DACH-style
/// accepted cost from `language.rs`'s module doc: a genuinely English-target
/// run against a German ad and a German source is NOT caught here — this
/// module goes quiet on a real detector/target disagreement rather than
/// guessing which side is right.
#[test]
fn language_critical_is_withheld_when_the_source_reads_the_same_way() {
    // Both documents are German; the target language says English.
    let report = report_for(DE_CLEAN, DE_SOURCE, DE_JOB_AD, &[]);
    assert!(
        !codes(&report).contains(&CONTENT_LANGUAGE_MISMATCH),
        "source and output detect identically — that is a detector disagreement, \
         not a generation defect; got {:?}",
        codes(&report)
    );
    assert!(
        report.metrics.keyword_coverage.is_some(),
        "the metrics must survive a withheld language Critical"
    );

    // The real defect — an English source, a German output — still fires.
    let real = en_resume(EN_WRONG_LANGUAGE, &en_requirements());
    let hits = fired(&real, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// **Advisory MEDIUM (confirmation review).** The DACH miss — the accepted
/// cost `language.rs:5-13`'s module doc describes in the abstract, pinned as
/// an actual scenario so a future change cannot quietly start believing this
/// guard is universal. An English-language job ad (realistic: an
/// international-facing posting scraped for a DACH-market role), an
/// untranslated English source, target `"de"`, and the model returns English
/// anyway — the SAME real defect [`regime_1_the_reported_bug_an_untranslated_english_resume_for_a_german_target_fires`]
/// catches, just with an English ad instead of a German one. Nothing here
/// corroborates `"de"`: neither witness confidently reads as German, so
/// `target_is_corroborated` has no reliable premise for `"de"` at all — the
/// SAME "goes quiet on a real disagreement" posture as
/// [`language_critical_is_withheld_when_the_source_reads_the_same_way`], from
/// the other direction (there, both witnesses agreed with the OUTPUT; here,
/// both witnesses confidently name a DIFFERENT real language than the
/// target). Silent on BOTH the document AND section passes — but for two
/// SEPARATE reasons that only happen to coincide in this particular fixture,
/// not because one implies the other in general: `is_language_mismatch` on
/// the whole document is true here (English confidently isn't `"de"`), which
/// closes the document pass AND, in this specific case, the `is_language_mismatch`
/// half of [`section_language_issues`]'s own gate. That coincidence does NOT
/// generalize — it holds only when the WHOLE document confidently reads as
/// some other language. A document that drifts only PARTIALLY (ambiguous
/// whole-text confidence — the exact shape a real section-level drift
/// produces, and the one [`a_single_drifted_section_is_caught_even_though_the_document_reads_clean`]
/// pins) does not trip `is_language_mismatch` at all, so
/// [`section_language_issues`] would stay silent there purely on its OTHER,
/// independent condition — `target_is_corroborated` — which is exactly what
/// [`a_drifted_section_stays_quiet_when_the_target_language_is_not_corroborated`]
/// pins.
#[test]
fn a_confidently_english_ad_does_not_corroborate_a_german_target_the_dach_miss() {
    assert!(
        !document_language_mismatch(EN_CLEAN, EN_SOURCE, EN_JOB_AD, "de"),
        "premise: neither witness reads as German, so the target has no corroboration — \
         the accusation must stay quiet even though the document genuinely never got \
         translated"
    );
    let report = validate_content(&ContentInput {
        generated: EN_CLEAN, // never translated — the same shape as regime 1
        source_resume: EN_SOURCE,
        job_ad: EN_JOB_AD, // English, not German — the one variable that changes
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

// The six regimes hand-verified in the plan for this fix, each pinned as its
// own regression so a future change to `document_language_mismatch` cannot
// silently reintroduce the original bug (a translation run scored `criticals=0`
// on a fully-untranslated résumé) or reopen any of the false positives the old
// `source_is_a_reliable_control` gate existed to prevent.

/// Regime 1 — THE REPORTED BUG. A German ad, an English source (translation
/// expected), target `"de"`, and the model returns English anyway. Under the
/// OLD `source_is_a_reliable_control` gate this was DEAD BY CONSTRUCTION:
/// `languages_align(EN_SOURCE, "de")` is false, so the control never passed —
/// dead in the exact one scenario it exists for. `target_is_corroborated`
/// asks the AD instead (`detected_language(DE_JOB_AD) == Some("de")`), which
/// does not depend on whether a translation happened.
#[test]
fn regime_1_the_reported_bug_an_untranslated_english_resume_for_a_german_target_fires() {
    let report = validate_content(&ContentInput {
        generated: EN_CLEAN, // never translated — the reported defect
        source_resume: EN_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// Regime 2 — the SAME ad/source pair as regime 1, but the model did its job:
/// a genuine German translation. Must stay quiet — the fix must not turn every
/// correct translation into a false Critical.
#[test]
fn regime_2_a_correct_translation_for_the_same_ad_and_source_stays_quiet() {
    let report = validate_content(&ContentInput {
        generated: DE_CLEAN, // correctly translated
        source_resume: EN_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// Regime 3 — a same-language (no-translation) non-Latin run. This is a FIX,
/// not a regression pin: under the OLD `languages_align`-routed guard,
/// `is_language_mismatch(japanese_text, "ja")` was `true` unconditionally —
/// `languages_align`'s non-Latin arm returns `false` for EVERY locale,
/// including the script's own — so a same-language Japanese run raised a
/// false Critical purely because of which arm of `languages_align` fired, not
/// because anything was wrong. `detected_language` has a real `"ja"` arm.
#[test]
fn regime_3_a_same_language_japanese_run_no_longer_raises_a_false_critical() {
    let ja = "私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。 \
              私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。 \
              私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。 \
              私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。 \
              私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。";
    let report = report_in("ja", ja, ja, ja);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// Regime 4 — a target this crate does not curate (Polish is not one of the
/// nineteen `documents::keywords::locale_tag_of` languages). `detected_language`
/// answers `None` for Polish text no matter how confidently `whatlang` reads
/// it, so a correct Polish résumé must never earn a false Critical just
/// because the target language has no entry in the table.
#[test]
fn regime_4_an_uncurated_target_language_never_raises_a_false_critical() {
    let pl = "Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych. \
              Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych. \
              Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych.";
    assert_eq!(
        crate::documents::keywords::detected_language(pl),
        None,
        "premise: Polish is confidently read but not a curated tag — see \
         documents::keywords::test::detected_language_is_none_for_a_language_this_crate_does_not_curate"
    );
    let report = report_in("pl", pl, pl, pl);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// Regime 5 — a target `"en"` where BOTH witnesses confidently read as a
/// COVERED but WRONG language (French — the same lowercase tool-list misread
/// the Skills exclusion test above measures). Neither witness corroborates
/// `"en"`, so the accusation stays quiet even though the generated text is
/// genuinely German (a real mismatch, had `"en"` been corroborated). The
/// module's own doc comment frames this generically as a franc/whatlang
/// detector disagreement (the renderer's target-picker vs. this module's
/// validator); this is the concrete "both witnesses read as some OTHER
/// language" instance of that same accepted limit.
///
/// Mutation check: change `target_is_corroborated` to always return `true` —
/// RAN, went red (`content.language_mismatch` fired even though neither
/// witness actually corroborates "en"), reverted.
#[test]
fn regime_5_neither_witness_corroborating_the_target_stays_quiet() {
    let misread_as_french = LOWERCASE_TOOL_LIST;
    assert!(
        matches!(
            crate::documents::keywords::detected_language(misread_as_french),
            Some(found) if found != "en"
        ),
        "premise: whatlang confidently reads this text as some covered language OTHER than \
         the target \"en\" — which language it names is a whatlang implementation detail, \
         not this crate's contract; what regime 5 needs is that neither witness corroborates \
         \"en\""
    );
    let report = validate_content(&ContentInput {
        generated: EN_WRONG_LANGUAGE, // genuinely German — would fire if "en" were corroborated
        source_resume: misread_as_french,
        job_ad: misread_as_french,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// [`target_is_corroborated`] is private to `language.rs`; exercised here
/// through [`document_language_mismatch`] (re-exported crate-wide) rather than
/// duplicating a second access path to a private function.
///
/// Mutation check: change the `||` in `target_is_corroborated` to `&&` (i.e.
/// require BOTH witnesses to agree) and this goes red — the reported bug's
/// own regime (regime 1 above) only ever has ONE corroborating witness (the
/// ad; the source is untranslated English), so `&&` would silently
/// reintroduce the exact defect this fix exists to close.
#[test]
fn either_witness_alone_is_enough_to_corroborate_the_target() {
    // Corroborated by the ad ALONE (source is untranslated English) — regime
    // 1's own shape, confirmed again through the public entry point.
    assert!(document_language_mismatch(
        EN_CLEAN, EN_SOURCE, DE_JOB_AD, "de"
    ));

    // Corroborated by the SOURCE alone: the ad does not confidently read as
    // "de" at all (it confidently reads as French — an aggregator snippet in
    // the wrong locale is the realistic shape), but the candidate's own
    // German source résumé is enough on its own.
    let misread_as_french = LOWERCASE_TOOL_LIST;
    assert!(document_language_mismatch(
        EN_CLEAN,
        DE_SOURCE,
        misread_as_french,
        "de"
    ));
}

/// R5-F2 (superseded by `target_is_corroborated`) — the OLD single-source
/// control required the CANDIDATE'S OWN résumé specifically to vouch for the
/// target, so it failed OPEN whenever the source could not (too short to
/// detect, or misdetected as a third language) — even when the JOB AD
/// independently and confidently corroborated that SAME target. That was too
/// narrow a premise: an ad genuinely written in English is real evidence
/// "en" was a credible target, whether or not the candidate's own résumé is
/// long enough to read, or happens to be in French. `target_is_corroborated`
/// widened the control to EITHER witness on purpose — this is the
/// DACH/translation case from the OTHER direction, where the ad (not the
/// source) is the reliable witness, and it now correctly fires here instead
/// of staying silent. See
/// [`a_language_critical_needs_at_least_one_reliable_witness`] for the case
/// this test used to conflate with these two: NEITHER witness can vouch for
/// the target, which still stays quiet.
#[test]
fn a_confidently_reliable_job_ad_corroborates_even_when_the_source_cannot() {
    // Too short for the detector to read at all — the SOURCE alone cannot
    // decide, but the job ad is a full, confident "en" read on its own, so
    // the target is still credible and the real mismatch still fires.
    let short_source = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\nEngineer | Acme | 2021\n";
    let too_short_source_report = report_against(EN_WRONG_LANGUAGE, short_source);
    let hits = fired(&too_short_source_report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);

    // A source reading as a THIRD language (French) does not disqualify the
    // ad's corroboration either — the ad, not the source, is the witness that
    // matters in this pair.
    let third_language_source_report = report_against(EN_WRONG_LANGUAGE, FR_RESUME);
    let hits = fired(&third_language_source_report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);

    // The original defect this whole family guards — a long English source, a
    // German output — obviously still fires too.
    let real = en_resume(EN_WRONG_LANGUAGE, &en_requirements());
    let hits = fired(&real, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// The genuine "cannot decide" case R5-F2's fixtures no longer exercise: BOTH
/// witnesses fail. The source is too short to detect, AND the ad is the
/// documented keyword-soup shape (`documents::keywords::test::detected_language_goes_quiet_below_the_confidence_floor`
/// pins its confidence at ~0.08) — neither can vouch for `"en"`, so the
/// accusation has no reliable premise on either side and must stay quiet, the
/// same "goes quiet on a real disagreement" posture
/// [`language_critical_is_withheld_when_the_source_reads_the_same_way`] takes
/// from the other direction.
///
/// Mutation check: change `target_is_corroborated`'s `||` to always `true`
/// (i.e. drop corroboration entirely) and this goes red — the mismatch fires
/// on the German `generated` text alone.
#[test]
fn a_language_critical_needs_at_least_one_reliable_witness() {
    let short_source = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\nEngineer | Acme | 2021\n";
    let unreliable_ad = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    silent(
        &report_for(EN_WRONG_LANGUAGE, short_source, unreliable_ad, &[]),
        CONTENT_LANGUAGE_MISMATCH,
    );
}
