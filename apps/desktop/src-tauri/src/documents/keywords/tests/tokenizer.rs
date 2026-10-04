//! The tokenizer and stemming helpers: filtering, synonym collapse, the normalized-vs-stemmed split,
//! and the noise filters (numerics, URL crumbs, chart labels).

use super::*;

#[test]
fn keywords_filters_short_and_stopwords() {
    let stemmer = Stemmer::create(Algorithm::English);
    let kw = keywords("Rust and TypeScript with the React framework", &stemmer);
    assert!(kw.contains("rust"));
    assert!(kw.contains("typescript"));
    assert!(kw.contains("react"));
    assert!(kw.contains("framework"));
    assert!(!kw.contains("and"));
    assert!(!kw.contains("the"));
    assert!(!kw.contains("with"));
}

#[test]
fn synonyms_normalize_js_to_javascript_and_k8s_to_kubernetes() {
    let stemmer = Stemmer::create(Algorithm::English);
    for (jd, resume, what) in [
        (
            "JavaScript developer",
            "experienced JS engineer",
            "expected javascript stemmed in both jd and resume sets",
        ),
        (
            "Kubernetes orchestration",
            "k8s cluster management",
            "expected kubernetes stemmed in both",
        ),
    ] {
        let jd_kw = keywords(jd, &stemmer);
        let resume_kw = keywords(resume, &stemmer);
        assert!(
            jd_kw.intersection(&resume_kw).count() >= 1,
            "{what}; jd={:?} resume={:?}",
            jd_kw,
            resume_kw
        );
    }
}

#[test]
fn synonyms_normalize_cpp() {
    let stemmer = Stemmer::create(Algorithm::English);
    let kw_explicit = keywords("C++ developer", &stemmer);
    let kw_slash = keywords("C/C++ developer", &stemmer);
    assert!(
        kw_explicit.iter().any(|w| w == "cpp"),
        "expected cpp from C++ developer; got {:?}",
        kw_explicit
    );
    assert!(
        kw_slash.iter().any(|w| w == "cpp"),
        "expected cpp from C/C++ developer; got {:?}",
        kw_slash
    );
}

#[test]
fn short_terms_pass_through() {
    let stemmer = Stemmer::create(Algorithm::English);
    let kw = keywords("AWS GCP SQL Go developer", &stemmer);
    assert!(kw.iter().any(|w| w.contains("aws") || w == "aws"));
    assert!(kw.iter().any(|w| w.contains("gcp") || w == "gcp"));
    assert!(kw.iter().any(|w| w.contains("sql") || w == "sql"));
}

#[test]
fn filler_words_excluded() {
    let stemmer = Stemmer::create(Algorithm::English);
    let kw = keywords("experience required skills knowledge", &stemmer);
    assert!(
        kw.is_empty(),
        "expected all filler words filtered; remaining tokens: {:?}",
        kw
    );
}

#[test]
fn normalized_set_is_not_stemmed() {
    let norm = keywords_normalized("developers building applications");
    assert!(norm.contains("developers"));
    assert!(norm.contains("applications"));
    let stemmer = Stemmer::create(Algorithm::English);
    let stemmed = apply_stemmer(norm, &stemmer);
    assert!(stemmed.contains("develop"));
    assert!(stemmed.contains("applic"));
}

// --- new split-API tests ---

/// keywords_normalized must NOT stem; the raw lowercased token "javascript"
/// must survive unchanged even though the English Snowball stemmer would
/// reduce it (or it at least differs from the stemmed form for other words).
#[test]
fn normalized_does_not_stem() {
    let norm = keywords_normalized("JavaScript developer");
    // The un-stemmed token must be present.
    assert!(
        norm.contains("javascript"),
        "keywords_normalized must preserve the unstemmed token; got {:?}",
        norm
    );
    // Apply stemming and confirm the stemmed set differs (proving normalization
    // returned pre-stemming tokens for at least one word in the input).
    let stemmer = Stemmer::create(Algorithm::English);
    let stemmed = apply_stemmer(norm.clone(), &stemmer);
    // "developer" → "develop"; the sets should differ on that token.
    assert!(
        norm != stemmed,
        "apply_stemmer must change at least one token; norm={:?} stemmed={:?}",
        norm,
        stemmed
    );
    // "javascript" itself must NOT appear stemmed — Snowball English stems it
    // to "javascript" (no change), so the key check is that the raw token is
    // present in the normalized set BEFORE stemming.
    assert!(
        !norm.contains("develop"),
        "normalized set must not contain stemmed form 'develop'; got {:?}",
        norm
    );
}

/// apply_stemmer reduces ordinary English words (e.g. "developing" → "develop").
#[test]
fn apply_stemmer_stems_normal_words() {
    let stemmer = Stemmer::create(Algorithm::English);
    let tokens: HashSet<String> = ["developing".to_string()].into_iter().collect();
    let stemmed = apply_stemmer(tokens, &stemmer);
    assert!(
        stemmed.contains("develop"),
        "expected 'developing' to be stemmed to 'develop'; got {:?}",
        stemmed
    );
    assert!(
        !stemmed.contains("developing"),
        "stemmed set must not contain the original form; got {:?}",
        stemmed
    );
}

/// Short tech terms bypass stemming so acronyms are not mangled (e.g. "aws"
/// would become "aw" under English Snowball without the bypass).
#[test]
fn apply_stemmer_bypasses_short_tech_terms() {
    let stemmer = Stemmer::create(Algorithm::English);
    let tokens: HashSet<String> = ["aws", "gcp", "cpp"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let stemmed = apply_stemmer(tokens, &stemmer);
    assert!(
        stemmed.contains("aws"),
        "aws must pass through unchanged; got {:?}",
        stemmed
    );
    assert!(
        stemmed.contains("gcp"),
        "gcp must pass through unchanged; got {:?}",
        stemmed
    );
    assert!(
        stemmed.contains("cpp"),
        "cpp must pass through unchanged; got {:?}",
        stemmed
    );
    assert_eq!(stemmed.len(), 3, "no extra tokens; got {:?}", stemmed);
}

/// `keywords_normalized` must stay a pure `collect()` over
/// `keywords_normalized_list` — one tokenizer, two shapes. If someone
/// re-implements either side, the sets diverge and this fails.
#[test]
fn normalized_list_collects_to_the_normalized_set() {
    let text = "Rust and rust with TypeScript, TypeScript and AWS aws experience";
    let from_list: HashSet<String> = keywords_normalized_list(text).into_iter().collect();
    assert_eq!(from_list, keywords_normalized(text));
}

/// The list form keeps duplicates (that is its whole reason to exist) while
/// the set form collapses them.
#[test]
fn normalized_list_preserves_repeats() {
    let list = keywords_normalized_list("rust rust rust docker");
    assert_eq!(
        list.iter().filter(|t| *t == "rust").count(),
        3,
        "repeats must survive in the list form; got {list:?}"
    );
    assert_eq!(
        keywords_normalized("rust rust rust docker").len(),
        2,
        "the set form still deduplicates"
    );
}

/// Round-trip invariant: apply_stemmer(keywords_normalized(text), stemmer)
/// must equal keywords(text, stemmer) for any input.
#[test]
fn keywords_normalized_then_apply_stemmer_equals_keywords() {
    let text = "Experienced JavaScript developer building TypeScript APIs on AWS";
    let stemmer = Stemmer::create(Algorithm::English);
    let round_trip = apply_stemmer(keywords_normalized(text), &stemmer);
    let direct = keywords(text, &stemmer);
    assert_eq!(
        round_trip, direct,
        "round-trip must equal keywords(); round_trip={:?} direct={:?}",
        round_trip, direct
    );
}

/// Pure-numeric tokens (postcodes, bare years) are dropped everywhere,
/// regardless of detected language; alphanumeric tech tokens with at
/// least one non-digit character are untouched.
///
/// Mutation check (performed, not hypothetical): removing the
/// `!s.chars().all(|c| c.is_ascii_digit())` clause from
/// `keywords_normalized_list` turns this red (`13385` and `2026` survive).
/// Reverted after confirming.
#[test]
fn pure_numeric_tokens_dropped_alphanumeric_tech_tokens_survive() {
    let text = "Postal code 13385, hiring for 2026. Skills: c4, s3, oauth2, es2015.";
    let kw = keywords_normalized(text);
    assert!(
        !kw.contains("13385"),
        "pure-numeric postcode must be dropped; got {kw:?}"
    );
    assert!(
        !kw.contains("2026"),
        "pure-numeric year must be dropped; got {kw:?}"
    );
    for tech in ["oauth2", "es2015"] {
        assert!(
            kw.contains(tech),
            "mixed alphanumeric tech token {tech:?} must survive; got {kw:?}"
        );
    }
    // c4/s3 are 2 chars and not in SHORT_TECH_TERMS, so they were already
    // dropped by the length filter before this change — assert that
    // pre-existing behavior is unaffected, not newly broken.
    assert!(!kw.contains("c4"));
    assert!(!kw.contains("s3"));
}

/// Issue #1223: slash-shaped tokens that the tokenizer's slash-tolerant split keeps as ONE
/// token (`/company/harvey`) are scraped-chrome noise and must NOT surface as fake skills —
/// while the slashed SYNONYMS (`ci/cd` → `cicd`, `c/c++` → `cpp`) must still survive.
///
/// Mutation check (performed, not hypothetical): removing the
/// `!has_path_separator(s)` clause from `normalize_list_with_stopwords` turns
/// this red (`/company/harvey` survives). Reverted after confirming.
#[test]
fn path_separator_tokens_dropped_slashed_synonyms_survive() {
    let kw = keywords_normalized("Skills: ci/cd and C/C++. More at /company/harvey");
    assert!(
        kw.contains("cicd"),
        "ci/cd synonym must survive; got {kw:?}"
    );
    assert!(kw.contains("cpp"), "c/c++ synonym must survive; got {kw:?}");
    assert!(
        !kw.iter().any(|w| w.contains('/')),
        "no slash-token may survive; got {kw:?}"
    );
}

/// Issue #1223: chart/axis date labels from scraped job-ad chrome (`1sep`, `2025mar`,
/// `2026sep` — 1-4 digits glued to a 3-letter month abbreviation, either order) must not
/// surface as fake skills — while real version tokens with a digit run glued to a NON-month
/// core (`es2015`, `oauth2`) stay untouched.
///
/// Mutation check (performed, not hypothetical): removing the
/// `!is_chart_date_label(s)` clause from `normalize_list_with_stopwords` turns
/// this red (`2026sep`/`1sep` survive). Reverted after confirming.
#[test]
fn chart_date_labels_dropped_tech_version_tokens_survive() {
    let kw =
        keywords_normalized("2026sep hiring push, 1sep chart, skills: es2015 and oauth2, react17");
    assert!(
        !kw.contains("2026sep"),
        "digit-then-month chart label must be dropped; got {kw:?}"
    );
    assert!(
        !kw.contains("1sep"),
        "digit-then-month chart label must be dropped; got {kw:?}"
    );
    for tech in ["es2015", "oauth2", "react17"] {
        assert!(
            kw.contains(tech),
            "real tech token {tech:?} must survive; got {kw:?}"
        );
    }
}
