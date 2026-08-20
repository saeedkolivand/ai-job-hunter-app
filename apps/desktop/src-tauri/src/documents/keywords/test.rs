//! Unit tests for `documents::keywords` — the tokenizer, the language-aware
//! stopword selection, the numeric-token filter and the coverage math.
//!
//! Split into a sibling file (the `commands/match_resume.rs` + `match_resume/`
//! precedent) purely to keep the parent module under R8's LOC cap; nothing about
//! the tests themselves changed in the move.

use super::*;

/// `detected_language` must not duplicate whatlang's own reliability
/// arithmetic in a second, hand-rolled comparison that can silently drift
/// out of step with it — it delegates to `Info::is_reliable()` directly
/// (see that function's doc comment). Two things pinned here:
///
/// * whatlang's own boundary is `confidence() > 0.9`, STRICTLY greater,
///   not `>=` — a confidence of exactly `0.9` is NOT reliable, while
///   anything above it is. That is the exact distinction the OLD code
///   (`info.confidence() < MIN_DETECTION_CONFIDENCE`) got backwards,
///   silently ACCEPTING a confidence of precisely `0.9`.
/// * [`MIN_DETECTION_CONFIDENCE`] itself stays in step with whatlang's
///   real, live threshold: both assertions probe `Info::is_reliable()` at
///   OUR const's value, so if a future edit ever moved the const away
///   from whatlang's actual `0.9` (in EITHER direction) one of the two
///   assertions flips.
///
/// Mutation check (performed, not hypothetical): raising
/// `MIN_DETECTION_CONFIDENCE` to `0.95` turns the first assertion red
/// (`at_the_bar.is_reliable()` becomes `true` at 0.95, since whatlang's
/// real bar is still 0.9); reverting to `0.9` turns it green again.
///
/// What this test does NOT (and, short of finding real text whatlang
/// scores at exactly `0.9`, cannot) prove in a black-box way: that
/// [`detected_language`] itself still calls `is_reliable()` rather than a
/// reintroduced hand-rolled copy — for every OTHER confidence value the
/// two formulations agree, so no ordinary fixture can tell them apart
/// (verified: reverting `detected_language` to the old `confidence() <
/// MIN_DETECTION_CONFIDENCE` comparison does NOT turn any test in this
/// module red, this one included). The delegation itself is enforced by
/// reading the one line of source next to this test, the same "true by
/// construction, not by a coincidental second number" argument the doc
/// comment above makes.
#[test]
fn whatlang_reliability_boundary_is_strictly_greater_than_0_9() {
    let at_the_bar =
        whatlang::Info::new(whatlang::Script::Latin, Lang::Eng, MIN_DETECTION_CONFIDENCE);
    assert!(
        !at_the_bar.is_reliable(),
        "whatlang's own bar is confidence > 0.9, not >=; exactly 0.9 must not be reliable"
    );
    let just_above = whatlang::Info::new(
        whatlang::Script::Latin,
        Lang::Eng,
        MIN_DETECTION_CONFIDENCE + 0.0001,
    );
    assert!(just_above.is_reliable());
}

/// English plus the six Snowball languages: full-sentence prose, the shape
/// [`detected_language`] is actually asked about in practice, reads
/// confidently and correctly. Anchored to the exact tag, not to
/// `detect_locale_tag` agreeing with itself — a passing pair that
/// compared two derived values against each other would survive a
/// regression that broke both the same way.
#[test]
fn detected_language_identifies_english_and_the_six_snowball_languages() {
    let cases: &[(&str, &str)] = &[
        (
            "en",
            "The candidate has eight years of backend experience with payment systems.",
        ),
        (
            "de",
            "Die Kandidatin hat acht Jahre Erfahrung im Backend-Bereich mit Zahlungssystemen.",
        ),
        (
            "fr",
            "La candidate a huit ans d'expérience dans les systèmes de paiement back-end.",
        ),
        (
            "es",
            "La candidata tiene ocho años de experiencia en sistemas de pago de backend.",
        ),
        (
            "it",
            "La candidata ha otto anni di esperienza nei sistemi di pagamento backend.",
        ),
        (
            "pt",
            "A candidata tem oito anos de experiência em sistemas de pagamento de backend.",
        ),
        (
            "nl",
            "De kandidaat heeft acht jaar ervaring met backend-betalingssystemen.",
        ),
    ];
    for (expected, text) in cases {
        assert_eq!(
            detected_language(text),
            Some(*expected),
            "text {text:?} should confidently detect as {expected}"
        );
    }
}

/// All twelve non-Latin languages `detect_locale_tag` already enumerated —
/// script alone gives `whatlang` a strong, near-1.0-confidence signal, so
/// these must all clear [`MIN_DETECTION_CONFIDENCE`] too. Full coverage
/// (all 19 languages `locale_tag_of` curates, together with the six
/// Snowball languages above) is the mechanical guard for "consistent
/// across every language" — without it the next language added to
/// `locale_tag_of` could silently drift out of step with what
/// `detected_language` actually detects.
#[test]
fn detected_language_identifies_non_latin_scripts() {
    let cases: &[(&str, &str)] = &[
        ("zh", "我是一名后端工程师，在支付系统和容器平台方面工作了八年。"),
        ("ja", "私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。"),
        ("ko", "저는 8년 동안 결제 시스템과 컨테이너 플랫폼을 구축해 온 백엔드 엔지니어입니다."),
        ("vi", "Tôi là kỹ sư backend với tám năm kinh nghiệm trong các hệ thống thanh toán và nền tảng container."),
        ("th", "ฉันเป็นวิศวกรแบ็กเอนด์ที่มีประสบการณ์แปดปีในระบบชำระเงินและแพลตฟอร์มคอนเทนเนอร์"),
        ("ar", "أنا مهندس أنظمة خلفية لدي ثماني سنوات من الخبرة في أنظمة الدفع ومنصات الحاويات."),
        ("he", "אני מהנדס backend עם שמונה שנות ניסיון במערכות תשלומים ופלטפורמות מכולות."),
        ("hi", "मैं एक बैकएंड इंजीनियर हूं जिसके पास भुगतान प्रणालियों और कंटेनर प्लेटफार्मों में आठ साल का अनुभव है।"),
        ("bn", "আমি একজন ব্যাকএন্ড ইঞ্জিনিয়ার যার পেমেন্ট সিস্টেম এবং কন্টেইনার প্ল্যাটফর্মে আট বছরের অভিজ্ঞতা রয়েছে।"),
        ("tr", "Ödeme sistemleri ve konteyner platformlarında sekiz yıllık deneyime sahip bir backend mühendisiyim."),
        ("uk", "Я бекенд-інженер з восьмирічним досвідом роботи з платіжними системами та контейнерними платформами."),
        ("ru", "Я бэкенд-инженер с восьмилетним опытом работы с платёжными системами."),
    ];
    for (expected, text) in cases {
        assert_eq!(
            detected_language(text),
            Some(*expected),
            "text {text:?} should confidently detect as {expected}"
        );
    }
}

/// A language `whatlang` knows and reads confidently, but this crate has
/// no tag for (Polish, Swedish, Czech, Romanian, Greek, …) — the exact
/// false-Critical risk `detected_language` returning `"en"` here would
/// have created. `locale_tag_of` simply has no arm for `Lang::Pol`, so this
/// is `None` regardless of confidence.
#[test]
fn detected_language_is_none_for_a_language_this_crate_does_not_curate() {
    let polish = "Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych.";
    assert!(
        detect(polish)
            .is_some_and(|i| i.lang() == Lang::Pol && i.confidence() >= MIN_DETECTION_CONFIDENCE),
        "premise: whatlang must confidently read this as Polish, or the test proves nothing \
         about the uncovered-language branch specifically (vs. the confidence-floor branch)"
    );
    assert_eq!(detected_language(polish), None);
}

/// The two documented false-positive shapes from `validate::content`'s own
/// history — a keyword-soup job ad and a short certifications block —
/// read as a language with LOW confidence. `detected_language` must go
/// quiet on both, the same "goes quiet rather than guesses" posture as
/// every other check in this crate.
///
/// Mutation check: delete the `!info.is_reliable()` gate in
/// `detected_language` (i.e. fall straight through to `locale_tag_of`)
/// and this goes red — both texts resolve to a confident-looking but
/// wrong `Some(_)`.
#[test]
fn detected_language_goes_quiet_below_the_confidence_floor() {
    let terse_ad = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    let certs_block = "CERTIFICATIONS\nAWS Certified Solutions Architect - Professional (2022)\n\
        Google Cloud Professional Data Engineer (2023)\n\
        Certified Kubernetes Administrator CKA (2021)";
    for text in [terse_ad, certs_block] {
        let info = detect(text).expect("whatlang must produce SOME guess to prove this case");
        assert!(
            info.confidence() < MIN_DETECTION_CONFIDENCE,
            "premise: {text:?} must be a LOW-confidence read ({:.4}), or this test is not \
             exercising the confidence gate at all",
            info.confidence()
        );
        assert_eq!(detected_language(text), None, "text: {text:?}");
    }
}

/// `detect_locale_tag` (stemmer selection, unconditional) and
/// `detected_language` (identity, confidence-gated) share
/// [`locale_tag_of`]'s table by construction, but this pins the OBSERVABLE
/// contract rather than trusting the shared-code argument alone: whenever
/// `detected_language` confidently names a language, `detect_locale_tag`
/// must name the exact same one — the two may differ only when
/// `detected_language` goes quiet (low confidence, or an uncovered
/// language), where `detect_locale_tag` still has to pick SOME stemmer.
///
/// Mutation check: add a `.filter(|info| info.is_reliable())` gate to
/// `detect_locale_tag` (making it confidence-gated like `detected_language`)
/// — RAN, went red (`detect_locale_tag` fell back to "en" for the
/// low-confidence-but-covered fixture instead of picking the covered
/// language), reverted.
#[test]
fn detect_locale_tag_and_detected_language_agree_whenever_both_answer() {
    let samples = [
        "The candidate has eight years of backend experience with payment systems.",
        "Die Kandidatin hat acht Jahre Erfahrung im Backend-Bereich mit Zahlungssystemen.",
        "私はバックエンドエンジニアで、決済システムとコンテナプラットフォームの構築を8年間担当してきました。",
        "Terraform AWS PostgreSQL Kubernetes platform engineer",
        "Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych.",
    ];
    for text in samples {
        if let Some(identity) = detected_language(text) {
            assert_eq!(
                detect_locale_tag(text),
                identity,
                "detect_locale_tag and detected_language disagreed on {text:?}"
            );
        }
    }
    // And the always-picks-a-stemmer half: `detect_locale_tag` never goes
    // quiet, even where `detected_language` does. A LOW-confidence read
    // still names a covered language here (unconditional, by design —
    // see the doc comment); an UNCOVERED language (no `locale_tag_of`
    // arm at all) is the one case that still falls back to "en".
    let terse = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    let info = detect(terse).expect("whatlang must produce SOME guess to prove this case");
    assert!(
        info.confidence() < MIN_DETECTION_CONFIDENCE,
        "premise: {terse:?} must be a LOW-confidence read, or this no longer exercises the \
         confidence-agnostic half of detect_locale_tag"
    );
    // Derived, not hardcoded: which language whatlang names for this text is a whatlang
    // implementation detail (a version bump can shift its guess), not this crate's
    // contract. What the test needs is that whatlang's guess IS a covered language, so
    // "unconditional on confidence" is distinguishable from "always falls back to en".
    let expected = locale_tag_of(info.lang()).expect(
        "premise: whatlang's guess for this text must be a COVERED language, or this \
         assertion cannot tell 'unconditional on confidence' apart from 'always falls back \
         to en'",
    );
    assert_ne!(
        expected, "en",
        "premise: the covered language must differ from the fallback, or a confidence-gate \
         regression would coincidentally still agree with the unconditional answer"
    );
    assert_eq!(
        detect_locale_tag(terse),
        expected,
        "low-confidence but still a covered language — detect_locale_tag must still pick it"
    );
    assert_eq!(
        detect_locale_tag("Kandydatka ma osiem lat doświadczenia w systemach płatności backendowych."),
        "en",
        "Polish has no locale_tag_of arm at all, confidence aside — falls back to the English stemmer, unchanged from before this crate had a confidence gate"
    );
}

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
    let (kernel, _gaps) = keyword_coverage(&keywords(job, &stemmer), &keywords(resume, &stemmer))
        .expect("non-empty job must return Some");
    assert_eq!(coverage_score(resume, job), kernel);
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
