//! The distinctive-evidence gate: texts `whatlang` misreads that must stay quiet, and
//! nominal-register documents that must still fire.

use super::{support::*, *};

// This evidence mechanism was substantially REDESIGNED after an independent
// review measured three real defects in the first pass:
//
// 1. A single function-word pool pruned across all seven curated languages
//    dropped any word appearing in two or more lists — which left Spanish
//    (30 survivors) and Portuguese (33) too thin to evidence THEMSELVES, so a
//    genuinely Spanish/Portuguese document against a non-Spanish/Portuguese
//    target silently stopped raising a real Critical (a true-positive
//    regression). Fixed by making evidence PAIRWISE (`pairwise_evidence_count`):
//    a word is only pruned when the language it is being compared AGAINST
//    shares it, not when some unrelated third language does.
// 2. `ci`, `vi`, `io`, `ha`, `da`, `ti`, `os`, `em`, `am`, `im`, `zu`, `el`,
//    `na`, `ai`, `et`, `au` are genuine short function words in one curated
//    language AND common abbreviations/TLD fragments/editor names in free
//    text (`ci/cd`, the `vi` editor, `.io`, an HA cluster). A blanket
//    length-≥3 floor was tried FIRST and measured to also exclude Italian's
//    OWN core vocabulary (`il`, `la`, `di`, all 2 characters — see
//    `a_drifted_awards_section_warns_rather_than_blocks`, which briefly
//    regressed under that rule). A curated denylist was tried NEXT and also
//    REMOVED: the found-SPECIFIC pairwise match already closes every one of
//    the five reported cases on its own (none of those tokens is a word in
//    the language whatlang actually named), the denylist's only measured
//    trigger was an adversarial string of disconnected foreign idioms no
//    realistic document produces, and it cost real evidence from exactly the
//    languages fix 1 was written to protect (`el` is one of Spanish's
//    commonest words). What shipped is just a length-≥2 floor (drops bare
//    single letters only) plus curating "per" (a genuine English preposition)
//    into `FUNCTION_WORDS_EN` for the one 3-character false positive measured.
// 3. The section-scoped pass had its own, separate `detected_language`
//    comparison that never routed through the new evidence check at all —
//    fixed by having `section_language_issues` call the SAME
//    `is_language_mismatch` the document pass uses; see
//    `a_german_institution_name_inside_an_english_education_section_never_fires`
//    below for the regression pin that only a SECTION-scoped fixture can
//    exercise.
//
// A fourth, harder shape was also measured and is NOT a numeric-threshold
// problem: an otherwise-English document that merely NAMES a German or French
// institution can carry a few real foreign function words purely from that
// proper noun. A title-case-sandwich exclusion in `pairwise_evidence_count`
// was tried for this shape and REMOVED (it deleted nominal-register German's
// evidence wholesale); what closes it now is MIN_DISTINCTIVE_HITS alone —
// see its doc for the corpus and the accepted French/Italian residual risk.
// A second, COMPARATIVE bar (found's evidence must EXCEED target's, not
// merely clear a floor) was tried on top of the title-case exclusion and
// REMOVED: disabling it left every behavioural test in this file green, so
// it was carrying no load the floor does not already carry — see
// `distinctive_evidence_confirms_requires_the_shipped_floor` and the module
// doc for the measurement.

/// The exact reported false positive, reproduced directly. An honest ENGLISH
/// noun-phrase block — the shape an ordinary skills line or a terse CV takes
/// (measured: this exact list reads at `confidence() == 1.0`,
/// `is_reliable() == true`, `lang == Fra`) — starves `whatlang`'s n-gram
/// model of the function words it needs to read English from at all, and it
/// lands on some OTHER language with MAXIMUM confidence margin.
/// `is_language_mismatch` must not raise the deterministic Critical on it:
/// doing so blanks `keywordCoverage` and suppresses every alignment finding
/// on a document the user did nothing wrong with.
///
/// Mutation check (performed, not hypothetical): replaced
/// `distinctive_evidence_confirms(text, found, lang)` in `is_language_mismatch`
/// with a bare `true` (i.e. any confident Latin-script read counts, the
/// original two-condition body) — RAN, went red (this fixture raised
/// `content.language_mismatch` as a Critical), reverted.
#[test]
fn an_english_noun_phrase_block_never_earns_a_false_language_critical() {
    // The same lowercase tool/skill list `regime_5_neither_witness_...` and
    // the Skills-exclusion tests above already establish reads as SOME other
    // covered language — an ordinary skills line, no different in shape from
    // the reported bug's "Administration / Supervision / Coordination / …".
    let noun_phrases = LOWERCASE_TOOL_LIST;
    assert!(
        significant_chars(noun_phrases) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor, or this test proves nothing about \
         the confidence-margin gate specifically"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(noun_phrases),
        Some("fr"),
        "premise: whatlang must confidently (and wrongly) read this honest English \
         skills/tool list as French — measured directly at confidence 1.0, reliable — \
         or this test proves nothing about the OLD two-condition check firing here"
    );
    assert!(
        !is_language_mismatch(noun_phrases, "en"),
        "the distinctive-function-word gate must keep this quiet: a noun-phrase block \
         carries none of the real function words a genuine drift into another language \
         would leave behind"
    );

    // End-to-end via the public entry point, with real English witnesses on
    // both sides so the target is trivially corroborated — this is the shape
    // that used to blank keywordCoverage and every alignment finding.
    assert!(
        !document_language_mismatch(noun_phrases, EN_SOURCE, EN_JOB_AD, "en"),
        "a genuinely English noun-phrase document must not raise the deterministic \
         Critical just because whatlang misreads it"
    );
    let report = report_against(noun_phrases, EN_SOURCE);
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// Five tokens a review measured as false positives on the SAME lowercase
/// tool-list fixture this file already establishes reads as confident French:
/// `ci/cd` (Italian "ci"), `.io` domains (Italian "io"), the `vi` editor
/// (Italian "vi"), an "HA cluster" (Italian "ha"), and "cost per transaction"
/// (Italian "per" — closed by curating "per" into `FUNCTION_WORDS_EN`, a
/// genuine English preposition, so it collides out at the ordinary pairwise
/// step). None of these five tokens is actually a FRENCH word — whatlang's
/// guess never changes (`det=fr` throughout) — so `pairwise_evidence_count(text,
/// "fr", "en")` finds nothing regardless of what the OLD "any curated
/// language" pool found for a DIFFERENT language (Italian) whatlang never
/// named. This is the found-SPECIFIC pairwise match closing the leak entirely
/// on its own — no length floor or curated denylist for `ci`/`vi`/`io`/`ha`
/// was needed here or shipped: a denylist was tried and, after failing to find
/// a realistic document that needed it, removed (see the module doc for the
/// measurement).
#[test]
fn short_ambiguous_tokens_never_manufacture_evidence() {
    let base = LOWERCASE_TOOL_LIST;
    let cases: &[(&str, String)] = &[
        (
            "+ci/cd",
            format!("{base} ci/cd pipelines and ci/cd automation"),
        ),
        ("+.io", format!("{base} grafana.io and prometheus.io")),
        ("+vi", format!("{base} vi and emacs and vi macros")),
        ("+HA", format!("{base} HA cluster HA proxy")),
        (
            "+per",
            format!("{base} cost per transaction requests per second"),
        ),
    ];
    assert!(
        matches!(
            crate::documents::keywords::detected_language(base),
            Some(found) if found != "en"
        ),
        "premise: the base tool list must confidently misread as some OTHER covered \
         language, or the additions below prove nothing"
    );
    for (name, text) in cases {
        assert!(
            !document_language_mismatch(text, EN_SOURCE, EN_JOB_AD, "en"),
            "[{name}] must not manufacture a mismatch — none of these tokens are actually \
             a word in the language whatlang named"
        );
        let report = report_against(text, EN_SOURCE);
        silent(&report, CONTENT_LANGUAGE_MISMATCH);
    }
}

/// A pre-PR gate found that the title-case-sandwich exclusion (since
/// removed — see the module doc) deleted German's evidence wholesale,
/// because standard German CV register is NOMINAL ("Aufbau und Betrieb der
/// Zahlungsplattform"), not the finite-verb register
/// ([`EN_WRONG_LANGUAGE`]'s "Die Wartezeit wurde gesenkt…") every existing
/// wrong-language German fixture in this file happened to use. German
/// capitalises every noun, so in nominal register EVERY connector sits
/// between two capitalised words — the exclusion could not tell that apart
/// from a connector inside a proper noun, and this exact register never had
/// a regression pin. Document scope: the whole résumé is nominal.
#[test]
fn nominal_register_german_document_is_critical() {
    let de_nominal = "Jane Doe\n\
        jane.doe@example.com | +49 30 1234567 | github.com/janedoe\n\n\
        ZUSAMMENFASSUNG\n\n\
        Acht Jahre Erfahrung im Zahlungsverkehr und im Aufbau von Container-Plattformen.\n\n\
        BERUFSERFAHRUNG\n\n\
        Senior Backend Engineer | Acme Payments | 2021 - Present\n\
        - Aufbau und Betrieb der Zahlungsplattform mit Reduzierung der Wartezeit von 480ms \
        auf 90ms\n\
        - Leitung der Migration der Dienste auf einen Kubernetes-Cluster mit zwölftausend \
        Anfragen pro Sekunde\n\
        - Neuentwicklung des Wiederholungsplaners in Rust zur Reduzierung fehlgeschlagener \
        Zahlungen\n\n\
        Backend Developer | Globex Logistics | 2018 - 2021\n\
        - Entwicklung der Abrechnungsschnittstelle in Python und PostgreSQL für vierzig \
        Lagerstandorte\n\
        - Migration der Flotte zu AWS mit Terraform als Bereitstellungswerkzeug\n\n\
        KENNTNISSE\n\n\
        Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis\n\n\
        AUSBILDUNG\n\n\
        BSc Informatik, TU Berlin, 2014 - 2018\n";
    assert!(
        significant_chars(de_nominal) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: fixture must clear the char floor"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(de_nominal),
        Some("de"),
        "premise: must confidently read as German"
    );
    assert!(
        is_language_mismatch(de_nominal, "en"),
        "nominal-register German must still be caught — this is the exact register the \
         title-case-sandwich exclusion silenced"
    );
    let report = en_resume(de_nominal, &en_requirements());
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
}

/// The SAME nominal register, drifted into only the EXPERIENCE section of an
/// otherwise-English résumé — the shape the reported defect actually takes,
/// and the shape [`a_single_drifted_section_is_caught_even_though_the_document_reads_clean`]
/// already pins for Italian. German had no equivalent: the document-level
/// vote must stay clean (an English majority hides one drifted section) and
/// the section-level pass must catch it — this is the ONE fixture that
/// exercises nominal-register German through the SECTION-scoped half of
/// `is_language_mismatch`, not just the whole-document half above.
#[test]
fn nominal_register_german_section_is_critical() {
    let generated = EN_CLEAN.replace(
        "Senior Backend Engineer | Acme Payments | 2021 - Present\n\
        - Checkout latency fell from 480ms to 90ms once a Redis cache sat in front of the \
        ledger service\n\
        - Ran the Docker workloads on a Kubernetes cluster that answers 12000 requests \
        every second\n\
        - Rewrote the retry scheduler in Rust, and failed settlements fell by 35%\n\n\
        Backend Developer | Globex Logistics | 2018 - 2021\n\
        - 40 warehouse sites bill through an API I wrote in Python, backed by PostgreSQL\n\
        - Took the fleet to AWS, with the whole deployment pipeline written as Terraform",
        "Senior Backend Engineer | Acme Payments | 2021 - Present\n\
        - Aufbau und Betrieb der Zahlungsplattform mit Reduzierung der Wartezeit von 480ms \
        auf 90ms\n\
        - Leitung der Migration der Dienste auf einen Kubernetes-Cluster mit zwölftausend \
        Anfragen pro Sekunde\n\
        - Neuentwicklung des Wiederholungsplaners in Rust zur Reduzierung fehlgeschlagener \
        Zahlungen\n\n\
        Backend Developer | Globex Logistics | 2018 - 2021\n\
        - Entwicklung der Abrechnungsschnittstelle in Python und PostgreSQL für vierzig \
        Lagerstandorte\n\
        - Migration der Flotte zu AWS mit Terraform als Bereitstellungswerkzeug",
    );
    assert!(
        !is_language_mismatch(&generated, "en"),
        "premise: the document-level majority vote must NOT fire — one drifted nominal- \
         register German EXPERIENCE section inside an otherwise-English résumé is exactly \
         the case that hides from a whole-document read"
    );
    let report = en_resume(&generated, &en_requirements());
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
    assert_eq!(
        hits[0].section.as_deref(),
        Some("EXPERIENCE"),
        "the finding must name the drifted section, not just the document"
    );
}

/// The differentiator BLOCKING 3 asked for: a fixture where reverting ONLY
/// `section_language_issues`'s call to `is_language_mismatch` back to its
/// pre-fix raw `detected_language(&body) != ctx.lang` comparison turns this
/// test red while every OTHER section-level guard (`SectionKind::Skills`
/// exclusion, `looks_like_prose`, the confidence floor) stays satisfied — so
/// this is the ONE fixture in the suite that actually exercises the
/// section-scoped evidence branch, not just the document-scoped one.
///
/// The shape is realistic, not adversarial: an EDUCATION entry (a real
/// `SectionKey`, unlike Certifications/Awards, so a false positive here is
/// NOT downgraded to a Warning) that spells out a German institution's own,
/// untranslated name alongside genuine English degree labels — exactly what
/// a truthful English résumé for a candidate who studied in Germany looks
/// like. Measured: `detected_language` reads the section body as confident
/// German (`Some("de")`, conf 1.0) purely from the institution's own
/// connector words ("der", "und") — the OLD raw check fires on this alone.
///
/// Mutation check (performed, not hypothetical): reverted the
/// `section_language_issues` hunk in `language.rs` to
/// `if !matches!(detected_language(&body), Some(found) if found != ctx.lang) { return None; }`
/// (dropping the route through `is_language_mismatch`), leaving the
/// document-level pass untouched — RAN, went red (`content.language_mismatch`
/// fired on the EDUCATION section), reverted. All 211
/// `validate::content` tests that existed before this fix stayed green
/// through that same reversion, which is exactly the coverage gap BLOCKING 3
/// named.
#[test]
fn a_german_institution_name_inside_an_english_education_section_never_fires() {
    let section_body = "EDUCATION\n\
        Diploma in Business Administration, Ludwig-Maximilians-Universitat Munchen der \
        Isotopenforschung und Angewandten Wissenschaften\n\
        Certificate in Software Architecture, Technische Universitat Munchen der \
        Angewandten Wissenschaften\n";
    assert!(
        significant_chars(section_body) >= MIN_CHARS_FOR_LANGUAGE_CHECK,
        "premise: the section body must clear the per-section char floor"
    );
    assert!(
        looks_like_prose(section_body),
        "premise: this section must clear the prose-ratio filter, or the Skills/prose \
         guards alone would explain the silence and this test would prove nothing about \
         the evidence branch specifically"
    );
    assert_eq!(
        crate::documents::keywords::detected_language(section_body),
        Some("de"),
        "premise: whatlang must confidently (and wrongly) read this section as German — \
         the exact read the OLD raw section-level check trusted unconditionally"
    );
    assert!(
        !is_language_mismatch(section_body, "en"),
        "the SAME evidence gate the document pass uses must also keep the section pass \
         quiet here"
    );

    let generated = EN_CLEAN.replace(
        "BSc Computer Science, TU Berlin, 2014 - 2018",
        "Diploma in Business Administration, Ludwig-Maximilians-Universitat Munchen der \
         Isotopenforschung und Angewandten Wissenschaften\n\
         Certificate in Software Architecture, Technische Universitat Munchen der \
         Angewandten Wissenschaften",
    );
    assert!(
        generated != EN_CLEAN,
        "premise: the education entry must actually be replaced — EN_CLEAN's wording drifted, \
         so this test is now asserting silence on an unmodified English fixture"
    );
    let report = en_resume(&generated, &en_requirements());
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// The narrower cousin of the test above: the SAME shape under a heading
/// `classify_section` files as `Other` (Certifications has no `SectionKey`),
/// so even where the evidence gate did NOT exist this would only ever
/// downgrade to a Warning, never block. Pinned separately so a future reader
/// can see the severity difference is real, not asserted from the EDUCATION
/// case alone.
#[test]
fn a_german_institution_name_inside_an_english_certifications_section_never_fires() {
    assert_eq!(
        crate::documents::evidence::classify_section("CERTIFICATIONS"),
        crate::documents::evidence::SectionKind::Other,
        "premise: Certifications must classify as Other, or the severity-downgrade half \
         of this test's premise does not hold"
    );
    let certs = "\n\nCERTIFICATIONS\n\n\
        Diploma in Business Administration, Ludwig-Maximilians-Universitat Munchen der \
        Isotopenforschung und Angewandten Wissenschaften\n\
        Certificate in Software Architecture, Technische Universitat Munchen der \
        Angewandten Wissenschaften\n\
        Award for Outstanding Academic Performance, Deutsche Gesellschaft fur Informatik \
        und Datenverarbeitung\n";
    let generated = format!("{EN_CLEAN}{certs}");
    let report = en_resume(&generated, &en_requirements());
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}

/// The design question's OTHER measured shape: the REALISTIC one, matching
/// this crate's own fixture convention for an education entry — a terse
/// "Degree, Institution, Dates" one-liner (see `EN_SOURCE`/`DE_SOURCE`'s "BSc
/// Computer Science, TU Berlin, 2014 - 2018") — with a REAL, not fabricated,
/// German institution name whose OFFICIAL name genuinely contains "für" and
/// "und" (Fachhochschule für Technik und Wirtschaft Berlin is a real Berlin
/// university of applied sciences). A short institution name contributes at
/// most one or two hits, and real terse entries do not accumulate enough
/// evidence to clear `MIN_DISTINCTIVE_HITS` in the first place — `MIN_DISTINCTIVE_HITS`
/// is now the only thing keeping this fixture quiet, which makes it a direct
/// floor-calibration pin, worth keeping separate from the denser,
/// multi-sentence-labelled fixture above.
#[test]
fn a_terse_real_german_institution_name_never_fires() {
    let generated = EN_CLEAN.replace(
        "BSc Computer Science, TU Berlin, 2014 - 2018",
        "MSc Computer Science, Fachhochschule fur Technik und Wirtschaft Berlin, 2018 - 2020\n\
         BSc Computer Science, Technische Universitat Munchen, 2014 - 2018",
    );
    assert!(
        generated != EN_CLEAN,
        "premise: the terse institution entry must actually be replaced — EN_CLEAN's wording \
         drifted, so this test is now asserting silence on an unmodified English fixture"
    );
    let report = en_resume(&generated, &en_requirements());
    silent(&report, CONTENT_LANGUAGE_MISMATCH);
}
