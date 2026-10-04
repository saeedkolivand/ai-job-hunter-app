//! `detected_language` / `detect_locale_tag`: the confidence-gated language-identity answer and the
//! unconditional stemmer-selection answer, and how they relate.

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
const SNOWBALL_TEXTS: &[(&str, &str)] = &[
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

/// All twelve non-Latin languages `detect_locale_tag` already enumerated —
/// script alone gives `whatlang` a strong, near-1.0-confidence signal, so
/// these must all clear [`MIN_DETECTION_CONFIDENCE`] too. Full coverage
/// (all 19 languages `locale_tag_of` curates, together with the six
/// Snowball languages above) is the mechanical guard for "consistent
/// across every language" — without it the next language added to
/// `locale_tag_of` could silently drift out of step with what
/// `detected_language` actually detects.
const NON_LATIN_TEXTS: &[(&str, &str)] = &[
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

/// Every language `locale_tag_of` curates — the English + six Snowball prose above and the
/// non-Latin scripts — detects confidently as its own tag.
#[test]
fn detected_language_identifies_every_curated_language() {
    for (expected, text) in SNOWBALL_TEXTS.iter().chain(NON_LATIN_TEXTS) {
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
