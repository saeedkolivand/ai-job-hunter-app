//! Language-aware stopwords: the German posting defect, the curated per-language lists wired through
//! `language_profile`, and why fragment tokenizers must pin the language explicitly.

use super::*;
use crate::documents::keywords::stopwords_germanic::STOPWORDS_DE;

// --- language-aware stopwords (German posting defect) ---

/// A realistic German job posting, including the exact defect words from
/// the bug report (`abgeschlossenes`, `abgestimmt`, `abseits`,
/// `abwechslungsreiche`, `abhängig`) and a Berlin postcode (`13385`),
/// alongside real skill/domain keywords (`react`, `typescript`, `docker`,
/// `kubernetes`, `aws`, `softwareentwickler`, `informatik`).
const GERMAN_JD: &str =
    "Wir suchen erfahrene Softwareentwickler (m/w/d) für unser Team in Berlin 13385.

Aufgaben:
Entwicklung moderner Webanwendungen mit React, TypeScript und Docker.
Betrieb von Kubernetes-Clustern in der AWS Cloud.
Eng abgestimmt mit den Kollegen arbeiten, auch abseits der Kernarbeitszeit.

Anforderungen:
Abgeschlossenes Studium der Informatik oder eine abgeschlossene Ausbildung.
Erfahrung mit React und TypeScript.
Kenntnisse in Docker und Kubernetes.
Abwechslungsreiche Aufgaben, abhängig von der jeweiligen Kernzeit.

Wir bieten ein motiviertes Team.";

/// Pre-fix tokenization: same length/short-tech gate + synonym collapse as
/// [`keywords_normalized_list`], but stopword-filtered ONLY against the
/// flat, English-only [`STOPWORDS`] — exactly what every caller got before
/// this module gained language-aware stopwords, and NO numeric filter.
/// Built from the SAME public consts ([`SYNONYMS`], [`SHORT_TECH_TERMS`],
/// [`STOPWORDS`]) so it cannot silently diverge from what they actually
/// contain; only the stopword SOURCE and the numeric filter differ from
/// the production function, which is exactly the axis under test.
fn old_style_keywords(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#' && c != '/')
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        .map(|w| {
            SYNONYMS
                .iter()
                .find(|(alias, _)| *alias == w.as_str())
                .map(|(_, canon)| canon.to_string())
                .unwrap_or(w)
        })
        .map(|w| w.trim_matches(|c: char| c == '+' || c == '#').to_string())
        .filter(|w| {
            let s = w.as_str();
            !w.is_empty()
                && (w.len() > 3 || SHORT_TECH_TERMS.contains(&s))
                && !STOPWORDS.contains(&s)
        })
        .collect()
}

/// The measured defect: a German posting's keyword denominator was
/// inflated by German function words/adjectives and a postcode that the
/// English-only `STOPWORDS` never covered, tanking real coverage
/// percentages (7% / 8% in production). This pins the fix on the exact
/// reported words, plus the postcode, plus a strict reduction in total
/// keyword count, while every real skill/domain term survives untouched.
///
/// Mutation check (performed, not hypothetical): reverting
/// `keywords_normalized_list` to filter against `STOPWORDS` unconditionally
/// (dropping the `language_profile` call and the numeric guard) turns this
/// red — every defect-word assertion fails because `new_kw` then equals
/// `old_kw`. Reverted after confirming.
#[test]
fn german_job_posting_defect_words_are_filtered_real_skills_survive() {
    let old_kw = old_style_keywords(GERMAN_JD);
    let new_kw = keywords_normalized(GERMAN_JD);

    let defect_words = [
        "abgeschlossenes",
        "abgestimmt",
        "abseits",
        "abwechslungsreiche",
        "abhängig",
        "13385",
    ];
    for word in defect_words {
        assert!(
            old_kw.contains(word),
            "premise: {word:?} must have inflated the PRE-FIX keyword set, or this \
             test is not exercising the reported defect; old={old_kw:?}"
        );
        assert!(
            !new_kw.contains(word),
            "{word:?} must be filtered from the German keyword set after the fix; \
             new={new_kw:?}"
        );
    }

    // Every real skill/domain term the JD actually asks for must survive
    // untouched — the fix must not silently delete signal along with filler.
    let real_skills = [
        "softwareentwickler",
        "react",
        "typescript",
        "docker",
        "kubernetes",
        "aws",
        "informatik",
    ];
    for word in real_skills {
        assert!(
            new_kw.contains(word),
            "real skill/domain term {word:?} must survive the German stopword filter; \
             new={new_kw:?}"
        );
    }

    // The denominator must shrink, not just have some words swapped for
    // others — the whole point of the fix.
    assert!(
        new_kw.len() < old_kw.len(),
        "keyword count must collapse after language-aware stopwords; \
         old={} new={} old_set={old_kw:?} new_set={new_kw:?}",
        old_kw.len(),
        new_kw.len()
    );
}

/// A résumé sharing the JD's real skills scores meaningfully higher once
/// the denominator is not inflated by filler — the practical consequence
/// of the fix above, through the SAME `coverage_score` kernel
/// `commands::match_resume` and Autopilot use. Compares against an
/// honest pre-fix baseline computed with the same stemmer and the same
/// [`keyword_coverage`] formula (not a hardcoded magic threshold), so the
/// assertion tracks the real improvement rather than an arbitrary number
/// that could drift out of sync with the fixture.
#[test]
fn german_coverage_score_improves_once_denominator_is_not_inflated() {
    let german_resume = "Erfahrener Softwareentwickler mit mehrjähriger Erfahrung in \
         React, TypeScript, Docker und Kubernetes auf AWS Cloud-Infrastruktur.";

    let stemmer = make_stemmer(GERMAN_JD);
    let old_job_kw = apply_stemmer(old_style_keywords(GERMAN_JD), &stemmer);
    let old_resume_kw = apply_stemmer(old_style_keywords(german_resume), &stemmer);
    let (old_cov, _) = keyword_coverage(&old_job_kw, &old_resume_kw).expect("non-empty job set");

    let new_cov = coverage_score(german_resume, GERMAN_JD);

    assert!(
        new_cov > old_cov,
        "coverage must improve once the German denominator is not inflated by \
         filler; pre-fix (English-only stopwords) = {old_cov}%, post-fix = {new_cov}%"
    );
    assert!(
        new_cov >= old_cov * 1.25,
        "the improvement should be substantial (the production defect measured \
         7-8%, not a marginal few points), not just strictly positive; \
         pre-fix = {old_cov}%, post-fix = {new_cov}%"
    );
}

/// Hand-written, independently-authored membership list — NOT derived by
/// iterating [`STOPWORDS_DE`] itself, so an accidental deletion from the
/// const is caught here even though a loop over the const cannot catch it
/// (a loop only proves "every entry that IS there gets filtered").
#[test]
fn stopwords_de_hand_written_membership() {
    let expected = [
        "dass",
        "wenn",
        "aber",
        "oder",
        "sind",
        "haben",
        "werden",
        "können",
        "unser",
        "diese",
        "für",
        "über",
        "hinter",
        "unsere",
        "bereits",
        "erfahrung",
        "kenntnisse",
        "anforderungen",
        "aufgaben",
        "voraussetzungen",
        "wünschenswert",
        "verantwortlich",
        "abgeschlossenes",
        "abgestimmt",
        "abseits",
        "abwechslungsreiche",
        "abhängig",
    ];
    for word in expected {
        assert!(
            STOPWORDS_DE.contains(&word),
            "expected German stopword {word:?} missing from STOPWORDS_DE — hand-written \
             regression guard, independent of the const's own contents"
        );
    }
}

/// Every entry in [`STOPWORDS_DE`] is actually wired into the filter — a
/// content-correct list that never reaches `keywords_normalized_list`
/// would be silently useless. Pairs with the hand-written test above,
/// which catches the opposite failure (a word silently REMOVED from the
/// list without the wiring itself breaking).
#[test]
fn every_stopwords_de_entry_is_filtered_by_the_production_function() {
    // A document unambiguously German (so `language_profile` picks
    // STOPWORDS_DE) containing every stopword entry once, plus real skills.
    let doc = format!(
        "Wir suchen einen Softwareentwickler mit Erfahrung in React und Docker. {}",
        STOPWORDS_DE.join(" ")
    );
    let kw = keywords_normalized(&doc);
    for word in STOPWORDS_DE {
        assert!(
            !kw.contains(*word),
            "STOPWORDS_DE entry {word:?} was not filtered by keywords_normalized_list; \
             got {kw:?}"
        );
    }
    assert!(kw.contains("softwareentwickler"));
    assert!(kw.contains("react"));
    assert!(kw.contains("docker"));
}

/// Documents the three explicit judgment calls made while curating
/// `STOPWORDS_DE`: `agilen` (inflected `agil`/"agile") is a real
/// methodology skill signal, not filler; `academy` is an ambiguous
/// loanword/brand term; `analysierst` ("you analyze") names a real
/// action, not filler. All three must survive — "when unsure, leave it
/// in the keyword set".
#[test]
fn ambiguous_judgment_call_words_are_left_in_the_keyword_set() {
    let text = "Arbeiten in agilen Teams. Wir sind eine Academy für Data Science. \
                 Du analysierst komplexe Datensätze.";
    let kw = keywords_normalized(text);
    for word in ["agilen", "academy", "analysierst"] {
        assert!(
            kw.contains(word),
            "{word:?} is a judgment call this fix deliberately leaves IN the keyword \
             set (see STOPWORDS_DE's doc comment); got {kw:?}"
        );
    }
}

/// The English path must be unchanged: [`language_profile`] for English
/// (or any undetected/uncovered language) still resolves to
/// `(Algorithm::English, STOPWORDS)`, the exact pre-fix pair, and the
/// actual filtered output still drops the same English filler it always did.
#[test]
fn english_path_unchanged_by_language_aware_stopwords() {
    let english_jd = "We are looking for a Senior Backend Engineer with strong \
         experience in Rust, Docker and Kubernetes to join our growing team.";
    let (algo, stopwords) = language_profile(english_jd);
    assert!(matches!(algo, Algorithm::English));
    assert_eq!(
        stopwords, STOPWORDS,
        "English text must resolve to the original STOPWORDS list, unchanged"
    );

    let kw = keywords_normalized(english_jd);
    assert!(kw.contains("backend"));
    assert!(kw.contains("engineer"));
    assert!(kw.contains("rust"));
    assert!(kw.contains("docker"));
    assert!(kw.contains("kubernetes"));
    assert!(
        !kw.contains("looking"),
        "English filler must still be filtered"
    );
    assert!(
        !kw.contains("strong"),
        "English filler must still be filtered"
    );
    assert!(
        !kw.contains("team"),
        "English filler must still be filtered"
    );
    assert!(
        !kw.contains("join"),
        "English filler must still be filtered"
    );
}

/// Light coverage of the other five Snowball languages: at least one
/// curated stopword per language is dropped, while a shared tech token
/// (docker) survives — full parity with German is out of scope (German is
/// the measured defect), but each language must have SOME curated list
/// wired through `language_profile`.
#[test]
fn other_snowball_languages_have_curated_stopwords_wired() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "fr",
            "Nous recherchons un développeur avec de l'expérience en Docker et \
             Kubernetes pour notre équipe.",
            &["recherchons", "expérience", "équipe"],
        ),
        (
            "es",
            "Buscamos un desarrollador con experiencia en Docker y Kubernetes \
             para nuestro equipo.",
            &["buscamos", "experiencia", "equipo"],
        ),
        (
            "it",
            "Cerchiamo uno sviluppatore con esperienza in Docker e Kubernetes \
             per la nostra azienda.",
            &["cerchiamo", "esperienza", "azienda"],
        ),
        (
            "pt",
            "Procuramos um desenvolvedor com experiência em Docker e Kubernetes \
             para a nossa empresa.",
            &["procuramos", "experiência", "empresa"],
        ),
        (
            "nl",
            "Wij zoeken een ontwikkelaar met ervaring in Docker en Kubernetes \
             voor ons bedrijf.",
            &["zoeken", "ervaring", "bedrijf"],
        ),
    ];
    for (lang, text, expected_stopwords) in cases {
        let kw = keywords_normalized(text);
        for stopword in *expected_stopwords {
            assert!(
                !kw.contains(*stopword),
                "[{lang}] {stopword:?} must be filtered; got {kw:?}"
            );
        }
        assert!(
            kw.contains("docker"),
            "[{lang}] shared tech token 'docker' must survive; got {kw:?}"
        );
    }
}

/// The hazard that forces every fragment-tokenizing caller onto the `_for_lang`
/// variant — `validate::content::ats::keyword_density_issues` is one, and reading
/// its `ctx.lang` is what keeps it honest.
///
/// A skills-and-tooling résumé body is mostly language-neutral tokens, so
/// `whatlang`'s n-gram model has little to work with and misreads it. Here it
/// says **Dutch**, and `wurde` — German-only, Dutch is `werd` — survives that
/// profile while `STOPWORDS_DE` drops it. A caller that re-detects therefore gets
/// a different token set, a different denominator, and can flip a finding on or
/// off purely on a misdetection.
///
/// Deliberately tested HERE rather than end-to-end through the validator: to trip
/// the density ceiling a fixture needs the German-only word 7+ times, and that
/// many repeats add enough German signal to make detection CORRECT again, so the
/// end-to-end version can only ever pass for the wrong reason. Measured, after
/// writing that vacuous test three times.
#[test]
fn auto_detection_and_explicit_lang_disagree_on_a_tech_dense_body() {
    let body = "Rust Python Kubernetes Terraform Redis PostgreSQL Docker AWS Kafka Grafana
                Vite Turborepo pnpm Playwright Vitest ESLint Prettier Husky
                Die Plattform wurde migriert, die Pipeline wurde neu gebaut, das                 Monitoring wurde ergänzt";

    let detected = whatlang::detect(body).map(|i| i.lang());
    assert_ne!(
        detected,
        Some(whatlang::Lang::Deu),
        "fixture must MISdetect for this test to mean anything; got {detected:?}"
    );

    let auto = keywords_normalized_list(body);
    let explicit = keywords_normalized_list_for_lang(body, "de");
    let n_auto = auto.iter().filter(|t| t.as_str() == "wurde").count();
    let n_explicit = explicit.iter().filter(|t| t.as_str() == "wurde").count();

    assert!(
        n_auto > 0,
        "under the misdetected profile the German-only word must SURVIVE — that is          the hazard; got {n_auto}"
    );
    assert_eq!(
        n_explicit, 0,
        "under the resolved language it must be filtered; got {n_explicit}"
    );
}
