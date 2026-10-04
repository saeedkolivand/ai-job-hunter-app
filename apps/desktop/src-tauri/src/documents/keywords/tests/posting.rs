//! `markdown_to_plain` / `posting_text_blob`: URL crumbs and markdown syntax must never reach the ATS
//! keyword set, while real JD words (and multi-byte text) survive.

use super::*;

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
