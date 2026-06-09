//! Keyword extraction for ATS matching.
//!
//! The pipeline is split so cached tokens stay language-agnostic:
//! keywords_normalized does lowercase + synonym-collapse + filter (NO
//! stemming) and is what we persist per document; apply_stemmer stems a
//! normalized set with a stemmer whose language is detected at match time.
//! This lets the same cached resume tokens match a JD in any language.

use std::collections::HashSet;

use rust_stemmers::{Algorithm, Stemmer};
use whatlang::{detect, Lang};

pub const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "you",
    "your",
    "are",
    "our",
    "will",
    "have",
    "this",
    "that",
    "from",
    "they",
    "their",
    "them",
    "all",
    "but",
    "not",
    "who",
    "can",
    "out",
    "use",
    "any",
    "has",
    "had",
    "was",
    "were",
    "what",
    "when",
    "which",
    "while",
    "into",
    "over",
    "than",
    "such",
    "able",
    "work",
    "role",
    "team",
    "join",
    "must",
    "etc",
    "via",
    "per",
    // Job-ad filler that otherwise leaks into keyword sets.
    "looking",
    "experience",
    "strong",
    "good",
    "skills",
    "ability",
    "knowledge",
    "understanding",
    "including",
    "working",
    "related",
    "ensure",
    "within",
    "across",
    "multiple",
    "various",
    "required",
    "preferred",
    "plus",
    "bonus",
    "position",
    "candidate",
    "company",
    "responsibilities",
    "requirements",
    "qualifications",
    "benefits",
    "about",
    "like",
    "using",
    "build",
    "help",
    "make",
    "take",
    "great",
    "well",
    "also",
    "both",
    "each",
    "other",
    "need",
    "want",
    "year",
    "years",
];

/// Short technical terms that are real keywords but fall under the len > 3
/// filter - allowlisted so they survive tokenization.
pub const SHORT_TECH_TERMS: &[&str] = &[
    "go", "sql", "aws", "gcp", "css", "git", "api", "vue", "ios", "tdd", "bdd", "ci", "cd", "ml",
    "ai", "ui", "ux", "qa", "rx", "etl", "sap", "erp", "crm", "k8s", "r",
    // cpp is 3 chars - produced by the c plus plus synonym - must be allowlisted
    // or the len > 3 filter silently drops it.
    "cpp",
];

/// Alias to canonical form, applied before stemming so equivalent spellings of a
/// skill (for example js or javascript) collapse to one keyword on both sides.
pub const SYNONYMS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("golang", "go"),
    ("k8s", "kubernetes"),
    ("kube", "kubernetes"),
    ("node", "nodejs"),
    ("react.js", "react"),
    ("vue.js", "vue"),
    ("next.js", "nextjs"),
    ("nuxt.js", "nuxtjs"),
    ("psql", "postgresql"),
    ("postgres", "postgresql"),
    ("mongo", "mongodb"),
    ("tf", "tensorflow"),
    ("sklearn", "scikit-learn"),
    ("scikit", "scikit-learn"),
    ("ci/cd", "cicd"),
    ("c/c++", "cpp"),
    ("c++", "cpp"),
    ("objective-c", "objectivec"),
    ("llms", "llm"),
    ("genai", "generativeai"),
    ("gen-ai", "generativeai"),
];

/// Build a Snowball stemmer for the language detected in text, falling back to
/// English when detection is uncertain or the language is unsupported.
pub fn make_stemmer(text: &str) -> Stemmer {
    Stemmer::create(match detect(text).map(|i| i.lang()) {
        Some(Lang::Deu) => Algorithm::German,
        Some(Lang::Fra) => Algorithm::French,
        Some(Lang::Spa) => Algorithm::Spanish,
        Some(Lang::Ita) => Algorithm::Italian,
        Some(Lang::Por) => Algorithm::Portuguese,
        Some(Lang::Nld) => Algorithm::Dutch,
        _ => Algorithm::English,
    })
}

/// Normalize text to a language-agnostic keyword set: lowercase,
/// synonym-normalized, filtered - but NOT stemmed. Tokens shorter than 4 chars
/// are dropped unless they are in SHORT_TECH_TERMS; stopwords are excluded.
/// The slash is kept in tokenization so ci/cd survives as a single token.
///
/// Store this in the DB - apply apply_stemmer at match time to stay
/// language-agnostic (the stemmer language is detected from the JD, not the
/// resume, so caching a pre-stemmed set would bake in the wrong language).
pub fn keywords_normalized(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#' && c != '/')
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        // Synonym lookup runs on the raw lowercased token (before trim) so
        // entries like c-plus-plus map to cpp and still match - trim
        // would otherwise strip the trailing plus and make them dead code.
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

/// Stem a pre-normalized keyword set using the given stemmer.
/// SHORT_TECH_TERMS bypass stemming so e.g. the English Snowball plural rule
/// does not corrupt acronyms (aws becomes aw).
pub fn apply_stemmer(tokens: HashSet<String>, stemmer: &Stemmer) -> HashSet<String> {
    tokens
        .into_iter()
        .map(|w| {
            if SHORT_TECH_TERMS.contains(&w.as_str()) {
                w
            } else {
                stemmer.stem(&w).into_owned()
            }
        })
        .collect()
}

/// Convenience: normalize + stem in one call (used for JD keywords at match
/// time, and as the cache-miss fallback for resumes).
pub fn keywords(text: &str, stemmer: &Stemmer) -> HashSet<String> {
    apply_stemmer(keywords_normalized(text), stemmer)
}

#[cfg(test)]
mod test {
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
    fn synonyms_normalize_js_to_javascript() {
        let stemmer = Stemmer::create(Algorithm::English);
        let jd_kw = keywords("JavaScript developer", &stemmer);
        let resume_kw = keywords("experienced JS engineer", &stemmer);
        assert!(
            jd_kw.intersection(&resume_kw).count() >= 1,
            "expected javascript stemmed in both jd and resume sets; jd={:?} resume={:?}",
            jd_kw,
            resume_kw
        );
    }

    #[test]
    fn synonyms_normalize_k8s_to_kubernetes() {
        let stemmer = Stemmer::create(Algorithm::English);
        let jd_kw = keywords("Kubernetes orchestration", &stemmer);
        let resume_kw = keywords("k8s cluster management", &stemmer);
        assert!(
            jd_kw.intersection(&resume_kw).count() >= 1,
            "expected kubernetes stemmed in both; jd={:?} resume={:?}",
            jd_kw,
            resume_kw
        );
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
}
