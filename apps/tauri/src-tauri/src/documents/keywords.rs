//! Keyword extraction for ATS matching.
//!
//! The pipeline is split so cached tokens stay language-agnostic:
//! keywords_normalized does lowercase + synonym-collapse + filter (NO
//! stemming) and is what we persist per document; apply_stemmer stems a
//! normalized set with a stemmer whose language is detected at match time.
//! This lets the same cached resume tokens match a JD in any language.

use std::collections::{HashMap, HashSet};

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

/// Map each stemmed JD keyword to a human-readable display form, so the gaps
/// surfaced to the user read as real words ("kubernetes", "developer") instead
/// of Snowball stems ("kubernet", "develop").
///
/// The display form is the *unstemmed, normalized* token (lowercase, synonyms
/// collapsed) that stems to that key — synonym collapse means e.g. a `k8s` gap
/// surfaces as `kubernetes`. Best-effort: original casing from the raw JD is not
/// preserved (normalization lowercases), and if two distinct tokens stem to the
/// same key the first one encountered wins. The map keys are exactly the members
/// of `keywords(job_text, stemmer)`, so every gap has an entry.
pub fn display_forms(job_text: &str, stemmer: &Stemmer) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    // Iterate a sorted Vec, not the HashSet, so the `or_insert` winner for two
    // tokens sharing a stem is deterministic across runs.
    let mut tokens: Vec<_> = keywords_normalized(job_text).into_iter().collect();
    tokens.sort();
    for token in tokens {
        let stem = if SHORT_TECH_TERMS.contains(&token.as_str()) {
            token.clone()
        } else {
            stemmer.stem(&token).into_owned()
        };
        map.entry(stem).or_insert(token);
    }
    map
}

/// Replace each stemmed gap with its readable [`display_forms`] entry, falling
/// back to the stem itself if no mapping exists (should not happen, since the
/// map is keyed on the same JD keyword set). Order is preserved.
pub fn readable_gaps(gaps: &[String], display: &HashMap<String, String>) -> Vec<String> {
    gaps.iter()
        .map(|g| display.get(g).cloned().unwrap_or_else(|| g.clone()))
        .collect()
}

/// Keyword-coverage of a job's keyword set by a résumé's keyword set: the share
/// of job keywords (0–100, rounded) that also appear in the résumé, plus the
/// up-to-15 sorted missing keywords (`gaps`). Single source of the coverage
/// formula shared by the Jobs-page ATS sub-score ([`coverage_score`] /
/// `commands::match_resume::score_one`) and the headless Autopilot ranker.
/// Both sides are expected to be stemmed with the SAME (JD-derived) stemmer.
///
/// Returns `None` when the job keyword set is empty (sparse/unparseable posting)
/// so callers can distinguish "no extractable keywords" from "0% match".
pub fn keyword_coverage(
    job: &HashSet<String>,
    resume: &HashSet<String>,
) -> Option<(f64, Vec<String>)> {
    if job.is_empty() {
        return None;
    }
    let mut gaps: Vec<String> = job.difference(resume).cloned().collect();
    gaps.sort();
    let matched = job.len() - gaps.len();
    let coverage = (matched as f64 / job.len() as f64 * 100.0).round();
    gaps.truncate(15);
    Some((coverage, gaps))
}

/// Strip markdown syntax from a scoring text blob so URL fragments and
/// formatting tokens do not pollute the ATS keyword set.
///
/// Applied to the `description` field of `posting_text_blob` ONLY — the stored
/// `description` is not touched (the frontend renders that markdown).
///
/// Rules (applied in order):
/// 1. Inline links `[anchor](url)` → anchor text only (restores old HTML-strip
///    behaviour: href dropped, visible text kept).
/// 2. Bare URLs (`https?://…`) → removed (no visible text to keep).
/// 3. Heading markers (`# …`) → leading `#` characters stripped.
/// 4. `*` emphasis markers → removed. `_` is deliberately kept: underscores
///    are part of real tech tokens (`OPENAI_API_KEY`, `next_js`) and stripping
///    them blanket-corrupts ATS keyword extraction. Markdown `_` emphasis is
///    rare in scraped JD text and does not need dedicated removal here.
pub fn markdown_to_plain(text: &str) -> String {
    // Step 1: collapse `[anchor text](url)` → `anchor text`.
    // Operates entirely on &str slices (char-boundary-safe) — never byte as char.
    let no_links = {
        let mut out = String::with_capacity(text.len());
        let mut remaining = text;
        while let Some(open) = remaining.find('[') {
            // Emit the text before the `[`.
            out.push_str(&remaining[..open]);
            let after_open = &remaining[open + 1..]; // char after `[`
                                                     // Look for `](` that closes this link's anchor.
            if let Some(close_bracket) = after_open.find("](") {
                let anchor = &after_open[..close_bracket];
                let after_bracket = &after_open[close_bracket + 2..]; // past `](`
                if let Some(close_paren) = after_bracket.find(')') {
                    // Valid `[anchor](url)` — emit only the anchor text.
                    out.push_str(anchor);
                    remaining = &after_bracket[close_paren + 1..];
                    continue;
                }
            }
            // Not a valid link syntax — emit the `[` literally and advance past it.
            out.push('[');
            remaining = after_open;
        }
        // Emit whatever is left after the last `[` (or the whole string if none).
        out.push_str(remaining);
        out
    };

    // Step 2: remove bare URLs (`https?://` followed by non-whitespace chars).
    let no_urls = {
        let mut out = String::with_capacity(no_links.len());
        let mut remaining = no_links.as_str();
        while let Some(pos) = remaining.find("http") {
            let prefix = &remaining[..pos];
            out.push_str(prefix);
            let tail = &remaining[pos..];
            if tail.starts_with("https://") || tail.starts_with("http://") {
                // Skip all non-whitespace chars of the URL.
                let url_len = tail.find(|c: char| c.is_whitespace()).unwrap_or(tail.len());
                remaining = &tail[url_len..];
            } else {
                // "http" but not a full URL prefix — emit the char and advance.
                out.push('h');
                remaining = &tail[1..];
            }
        }
        out.push_str(remaining);
        out
    };

    // Step 3: strip leading heading markers (`# `, `## `, etc.) per line.
    // Step 4: remove `*` emphasis markers only — do NOT strip `_`.
    //
    // Underscores are part of real tech tokens (OPENAI_API_KEY, next_js,
    // MY_ENV_VAR). Stripping every `_` blanket-corrupts those tokens before
    // keyword extraction, breaking ATS matching. `_` as a markdown emphasis
    // delimiter is rare in scraped JD text, and even when present the
    // tokenizer in `keywords_normalized` already splits on non-alphanumeric
    // characters (excluding `_` is not needed there). Keep `_` intact.
    no_urls
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches('#').trim_start();
            trimmed.replace('*', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the ATS text blob for a job posting — title + description + requirements,
/// joined by newlines. Single source of truth shared by the Jobs-page scorer
/// (`commands::match_resume`) and the headless Autopilot ranker, so both score
/// identical text. Returns None when there's no usable text.
///
/// The `description` field is normalised with [`markdown_to_plain`] before
/// inclusion so markdown links and bare URLs do not inject URL-fragment tokens
/// (`https`, host segments, path segments) into the ATS keyword set. The stored
/// `description` value is not modified — this normalisation is scoring-blob only.
pub fn posting_text_blob(
    title: &str,
    description: Option<&str>,
    requirements: Option<&[String]>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !title.trim().is_empty() {
        parts.push(title.to_string());
    }
    if let Some(d) = description {
        let plain = markdown_to_plain(d);
        if !plain.trim().is_empty() {
            parts.push(plain);
        }
    }
    if let Some(reqs) = requirements {
        for r in reqs {
            if !r.trim().is_empty() {
                parts.push(r.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Embedding-free keyword-coverage match score (0–100) of a résumé against a
/// job's text. This is the SAME kernel as the Jobs-page ATS sub-score: detect
/// the stemmer language from the JD, extract+stem both sides, and report the
/// share of job keywords covered by the résumé. No embedding / API calls — safe
/// for the headless Autopilot scheduler.
///
/// Returns only the coverage percentage; callers that also need the missing
/// keywords should build the keyword sets and call [`keyword_coverage`].
pub fn coverage_score(resume_text: &str, job_text: &str) -> f64 {
    let stemmer = make_stemmer(job_text);
    let job_kw = keywords(job_text, &stemmer);
    let resume_kw = keywords(resume_text, &stemmer);
    // None → no extractable JD keywords; return 0.0 for the headless ranker
    // (Autopilot filters by minMatchScore, so 0.0 safely excludes sparse postings).
    keyword_coverage(&job_kw, &resume_kw).map_or(0.0, |(cov, _)| cov)
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

    fn set(words: &[&str]) -> HashSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn keyword_coverage_full_when_resume_has_all() {
        let job = set(&["rust", "react", "docker"]);
        let resume = set(&["rust", "react", "docker", "extra"]);
        let (cov, gaps) = keyword_coverage(&job, &resume).expect("non-empty job must return Some");
        assert_eq!(cov, 100.0);
        assert!(gaps.is_empty());
    }

    #[test]
    fn keyword_coverage_reports_sorted_gaps() {
        let job = set(&["rust", "react", "docker", "kubernetes"]);
        let resume = set(&["rust", "react"]);
        let (cov, gaps) = keyword_coverage(&job, &resume).expect("non-empty job must return Some");
        assert_eq!(cov, 50.0);
        assert_eq!(gaps, vec!["docker".to_string(), "kubernetes".to_string()]);
    }

    #[test]
    fn keyword_coverage_empty_job_returns_none() {
        // Empty JD keyword set → None (distinguishable from 0% real mismatch).
        assert!(
            keyword_coverage(&HashSet::new(), &set(&["rust"])).is_none(),
            "empty job keyword set must return None, not Some(0.0)"
        );
    }

    #[test]
    fn keyword_coverage_caps_gaps_at_fifteen() {
        let job: HashSet<String> = (0..30).map(|i| format!("skill{i:02}")).collect();
        let (cov, gaps) =
            keyword_coverage(&job, &HashSet::new()).expect("non-empty job must return Some");
        assert_eq!(cov, 0.0);
        assert_eq!(gaps.len(), 15, "gaps must be truncated to 15");
    }

    /// `coverage_score` is the embedding-free Jobs-page ATS kernel: a résumé that
    /// contains all the JD's keywords scores high; an unrelated one scores 0.
    #[test]
    fn coverage_score_matches_and_misses() {
        let full = coverage_score(
            "experienced rust kubernetes docker engineer",
            "rust kubernetes docker",
        );
        assert_eq!(full, 100.0, "résumé covering all JD keywords → 100");

        let none = coverage_score("java spring developer", "rust kubernetes docker");
        assert_eq!(none, 0.0, "no overlap → 0");

        let partial = coverage_score("rust developer", "rust kubernetes docker");
        assert!(
            partial > 0.0 && partial < 100.0,
            "partial overlap must be strictly between 0 and 100; got {partial}"
        );
    }

    /// `coverage_score` must agree with the underlying `keyword_coverage` kernel
    /// (single source of the formula — guards against the two drifting apart).
    #[test]
    fn coverage_score_agrees_with_keyword_coverage_kernel() {
        let resume = "rust developer with docker";
        let job = "rust kubernetes docker terraform";
        let stemmer = make_stemmer(job);
        let (kernel, _gaps) =
            keyword_coverage(&keywords(job, &stemmer), &keywords(resume, &stemmer))
                .expect("non-empty job must return Some");
        assert_eq!(coverage_score(resume, job), kernel);
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

    // --- markdown_to_plain + posting_text_blob regression tests ---

    /// URL-fragment tokens must NOT appear in the ATS keyword set when the JD
    /// description contains markdown links or bare URLs.
    ///
    /// Regression: htmd converts HTML→markdown, so `[Apply now](https://x.io/postings/123)`
    /// and bare `https://acme.example.com/jobs` were injecting tokens like `https`,
    /// `x`, `io`, `postings`, `acme`, `example` into the JD keyword set, causing
    /// an ~19pp ATS-coverage drop on any 2-link JD.
    ///
    /// After the fix: only anchor text ("apply now") and real JD words survive;
    /// URL-fragment tokens are absent.
    #[test]
    fn markdown_links_and_bare_urls_do_not_pollute_keyword_set() {
        let stemmer = Stemmer::create(Algorithm::English);

        // Description as it arrives after htmd HTML→markdown conversion.
        let description = "We need a backend engineer. [Apply now](https://x.io/postings/123) \
                           or visit https://acme.example.com/jobs for details.";

        // Build the blob through the production path (posting_text_blob applies
        // markdown_to_plain to the description before returning the blob).
        let blob = posting_text_blob("Backend Engineer", Some(description), None)
            .expect("non-empty blob must be Some");

        let kw = keywords(&blob, &stemmer);

        // URL-fragment tokens that must NOT appear:
        for bad in &["https", "http", "x", "io", "postings", "acme", "example"] {
            assert!(
                !kw.contains(*bad),
                "URL-fragment token '{bad}' must not appear in keyword set; got {kw:?}"
            );
        }
        // The path segment that looks like a word must also be absent:
        // "123" is numeric-only so it's dropped by alphanumeric tokenisation, but
        // "jobs" is 4 chars and would survive without the URL strip — assert it's gone.
        // Note: "jobs" is part of the URL path, not a real JD keyword here.
        // However "jobs" could be a real keyword in other contexts, so we verify it
        // appears only when it's a real JD word (here it's URL-only, so absent).
        assert!(
            !kw.contains("jobs"),
            "URL-path segment 'jobs' must not pollute the keyword set; got {kw:?}"
        );

        // Anchor text and real JD words MUST survive:
        // "apply" stems from "apply now" anchor text; "backend"/"engineer" are real.
        assert!(
            kw.iter().any(|w| w.starts_with("appl")),
            "anchor text 'apply' (or its stem) must survive in keyword set; got {kw:?}"
        );
        assert!(
            kw.iter()
                .any(|w| w.starts_with("backend") || w.starts_with("backEnd") || w == "backend"),
            "real JD word 'backend' must survive; got {kw:?}"
        );
    }

    /// markdown_to_plain: inline link collapses to anchor text only.
    #[test]
    fn markdown_to_plain_collapses_link_to_anchor() {
        let plain = markdown_to_plain("[Apply now](https://x.io/postings/123)");
        assert_eq!(plain.trim(), "Apply now");
        assert!(!plain.contains("https"));
        assert!(!plain.contains("x.io"));
        assert!(!plain.contains("postings"));
    }

    /// markdown_to_plain: bare URL is fully removed.
    #[test]
    fn markdown_to_plain_removes_bare_url() {
        let plain = markdown_to_plain("Visit https://acme.example.com/jobs for more.");
        assert!(!plain.contains("https"));
        assert!(!plain.contains("acme"));
        assert!(!plain.contains("example"));
        assert!(!plain.contains("jobs"));
        assert!(plain.contains("Visit"));
        assert!(plain.contains("for more"));
    }

    /// markdown_to_plain: heading markers are stripped, `*` emphasis removed,
    /// but `_` is preserved (underscores are real tech-token characters).
    #[test]
    fn markdown_to_plain_strips_headings_and_emphasis() {
        let input = "## Requirements\n**Strong** _communication_ skills";
        let plain = markdown_to_plain(input);
        assert!(plain.contains("Requirements"), "heading text must survive");
        assert!(!plain.contains("##"), "heading marker must be stripped");
        assert!(!plain.contains("**"), "bold markers must be removed");
        // `_` is intentionally kept — underscores are part of real tech tokens
        // (OPENAI_API_KEY, next_js). Markdown `_` emphasis removal is not done.
        assert!(plain.contains("Strong"), "bold text content must survive");
        assert!(
            plain.contains("communication"),
            "italic text content must survive"
        );
    }

    /// Underscores inside tech tokens must survive `markdown_to_plain` intact so
    /// ATS keyword extraction sees `OPENAI_API_KEY` and `next_js`, not the
    /// corrupted forms `OPENAIAPIKEY` / `nextjs`.
    #[test]
    fn markdown_to_plain_preserves_underscores_in_tech_tokens() {
        let plain =
            markdown_to_plain("Required: OPENAI_API_KEY env var and next_js framework knowledge");
        assert!(
            plain.contains("OPENAI_API_KEY"),
            "underscore-separated env-var token must survive; got: {plain:?}"
        );
        assert!(
            plain.contains("next_js"),
            "underscore-separated tech token must survive; got: {plain:?}"
        );
    }

    /// `OPENAI_API_KEY` must be tokenised into its component tokens (`openai`, `api`,
    /// `key`) by the tokenizer's underscore split — not collapsed into the unmatchable
    /// blob `openaiapikey` by premature underscore removal in `markdown_to_plain`.
    ///
    /// Root cause: the old code did `replace(['*', '_'], "")` in `markdown_to_plain`,
    /// which stripped `_` before the tokenizer ran.  That turned `OPENAI_API_KEY` →
    /// `openaiapikey` — one token that never matches any JD keyword.  The fix keeps
    /// `_` in the `markdown_to_plain` output; the tokenizer in `keywords_normalized`
    /// already splits on `_` (non-alphanumeric), so each component word is extracted
    /// and matched individually.
    #[test]
    fn tech_tokens_with_underscores_survive_keyword_extraction() {
        let stemmer = Stemmer::create(Algorithm::English);
        let desc = "Must have OPENAI_API_KEY configured.";
        let blob = posting_text_blob("Senior Engineer", Some(desc), None)
            .expect("non-empty blob must be Some");
        let kw = keywords(&blob, &stemmer);

        // Regression guard: the corrupted collapsed form must not appear.
        assert!(
            !kw.contains("openaiapikey"),
            "collapsed form 'openaiapikey' must NOT appear (regression guard); got {kw:?}"
        );
        // The component parts must be present, extracted by the underscore split.
        assert!(
            kw.iter().any(|w| w.starts_with("openai")),
            "'openai' component of OPENAI_API_KEY must be in keyword set; got {kw:?}"
        );
        assert!(
            kw.contains("api"),
            "'api' component of OPENAI_API_KEY must be in keyword set; got {kw:?}"
        );
    }

    /// Bare-URL-only description: after stripping, no real words remain, so the
    /// blob must be `None` (no usable text). This asserts the `None` branch
    /// explicitly so the URL-token-absent invariant is always enforced, not
    /// silently skipped when the blob happens to be `None`.
    #[test]
    fn url_heavy_jd_produces_none_blob() {
        // Bare URLs only, empty title — after markdown_to_plain strips the URLs
        // the description is whitespace-only, and the title is empty, so
        // posting_text_blob must return None.
        let bare_url_desc = "https://b.io/postings/123 https://acme.example.com/careers";
        let blob = posting_text_blob("", Some(bare_url_desc), None);
        assert!(
            blob.is_none(),
            "bare-URL-only description + empty title must yield None blob; got {blob:?}"
        );
    }

    /// When a real JD word accompanies the URLs, the blob is `Some` and the
    /// keyword set must contain the real word but no URL-fragment tokens.
    #[test]
    fn url_with_real_word_excludes_url_fragment_tokens() {
        let stemmer = Stemmer::create(Algorithm::English);
        // One real JD word ("engineer") alongside bare URLs.
        let desc = "engineer https://b.io/postings/123 https://acme.example.com/careers";
        let blob = posting_text_blob("", Some(desc), None)
            .expect("description with real word must yield Some blob");
        let kw = keywords(&blob, &stemmer);
        for bad in &[
            "https", "http", "postings", "acme", "example", "careers", "io",
        ] {
            assert!(
                !kw.contains(*bad),
                "URL-fragment token '{bad}' must not appear in keyword set; got {kw:?}"
            );
        }
        // The real JD word must survive.
        assert!(
            kw.iter().any(|w| w.starts_with("engin")),
            "real JD word 'engineer' (or its stem) must survive; got {kw:?}"
        );
    }

    /// German UTF-8 round-trip: markdown_to_plain must not corrupt multi-byte
    /// characters. Umlauts (ü, ä, ö) in the non-link portions of the text must
    /// survive byte-identical after stripping. A link elsewhere in the string
    /// must not corrupt the surrounding German text.
    ///
    /// Regression for the `bytes[i] as char` mojibake bug: the old byte-loop
    /// reinterpreted each UTF-8 byte as a Unicode scalar, turning `ü` (U+00FC,
    /// bytes 0xC3 0xBC) into `Ã¼`, so stemmer input was garbled and German
    /// keywords were silently dropped from the JD keyword set.
    #[test]
    fn markdown_to_plain_preserves_german_utf8() {
        let input = "Softwareentwickler für Berlin, gute Qualität — [mehr](https://x.io/p/1)";
        let plain = markdown_to_plain(input);

        // Umlauts must survive intact.
        assert!(
            plain.contains("für"),
            "markdown_to_plain must preserve 'für' (umlaut ü); got: {plain:?}"
        );
        assert!(
            plain.contains("Qualität"),
            "markdown_to_plain must preserve 'Qualität' (umlaut ä); got: {plain:?}"
        );
        assert!(
            plain.contains("Softwareentwickler"),
            "markdown_to_plain must preserve 'Softwareentwickler'; got: {plain:?}"
        );

        // The URL must be gone.
        assert!(
            !plain.contains("https"),
            "URL must be stripped; got: {plain:?}"
        );
        assert!(
            !plain.contains("x.io"),
            "URL host must be stripped; got: {plain:?}"
        );

        // The anchor text "mehr" must survive.
        assert!(
            plain.contains("mehr"),
            "anchor text 'mehr' must survive; got: {plain:?}"
        );

        // Keyword set must be byte-identical whether or not a markdown link is present.
        // A JD with the same German words but no link must produce the same keywords.
        let without_link = "Softwareentwickler für Berlin, gute Qualität — mehr";
        let stemmer = make_stemmer(input); // German stemmer from the original input
        let kw_with_link = keywords(&markdown_to_plain(input), &stemmer);
        let kw_without_link = keywords(without_link, &stemmer);
        assert_eq!(
            kw_with_link, kw_without_link,
            "keyword sets must be identical with and without the markdown link; \
             with_link={kw_with_link:?} without_link={kw_without_link:?}"
        );
    }
}
