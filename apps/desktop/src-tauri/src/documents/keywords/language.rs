//! Language detection for the keyword kernel: which Snowball stemmer and stopword list a text gets,
//! whether a posting and a résumé are close enough to share a stemmer, and the confidence-gated
//! language-IDENTITY answer the validators use.
//!
//! Split out of `documents/keywords.rs` (R8's hard LOC cap); everything here moved verbatim.
//! [`languages_align`] and [`detected_language`] ask different questions and must never drift —
//! read each doc before touching either.

use rust_stemmers::{Algorithm, Stemmer};
use whatlang::{detect, Lang};

use super::lexicon::STOPWORDS;
use super::stopwords_germanic::{STOPWORDS_DE, STOPWORDS_NL};
use super::stopwords_romance::{STOPWORDS_ES, STOPWORDS_FR, STOPWORDS_IT, STOPWORDS_PT};

/// The Snowball [`Algorithm`] AND stopword list for the language detected in
/// `text` — single detect()+mapping call site for both [`make_stemmer`] and
/// [`keywords_normalized_list`]'s stopword choice, so the two answers for the
/// SAME text can never independently drift the way `STOPWORDS` being
/// English-only silently did.
pub(super) fn language_profile(text: &str) -> (Algorithm, &'static [&'static str]) {
    match detect(text).map(|i| i.lang()) {
        Some(Lang::Deu) => (Algorithm::German, STOPWORDS_DE),
        Some(Lang::Fra) => (Algorithm::French, STOPWORDS_FR),
        Some(Lang::Spa) => (Algorithm::Spanish, STOPWORDS_ES),
        Some(Lang::Ita) => (Algorithm::Italian, STOPWORDS_IT),
        Some(Lang::Por) => (Algorithm::Portuguese, STOPWORDS_PT),
        Some(Lang::Nld) => (Algorithm::Dutch, STOPWORDS_NL),
        _ => (Algorithm::English, STOPWORDS),
    }
}

/// [`language_profile`]'s stopword table, keyed by an EXPLICIT ISO-639-1 tag
/// instead of `detect()` — see [`keywords_normalized_list_for_lang`] for why a
/// caller needs this. Falls back to the English [`STOPWORDS`] for any tag not
/// curated here, the same fallback [`language_profile`] uses for an
/// unrecognised or undetected language.
pub(super) fn stopwords_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => STOPWORDS_DE,
        "fr" => STOPWORDS_FR,
        "es" => STOPWORDS_ES,
        "it" => STOPWORDS_IT,
        "pt" => STOPWORDS_PT,
        "nl" => STOPWORDS_NL,
        _ => STOPWORDS,
    }
}

/// Build a Snowball stemmer for the language detected in text, falling back to
/// English when detection is uncertain or the language is unsupported.
pub fn make_stemmer(text: &str) -> Stemmer {
    Stemmer::create(language_profile(text).0)
}

/// Whether the job posting's language and the résumé's locale are close enough
/// that BOTH sides should be stemmed with the JD-derived stemmer.
///
/// When they diverge, both sides must stay **normalized-only** (unstemmed):
/// stemming one side alone mutates language-neutral tech tokens (`docker`,
/// `kubernetes`) on that side only, so they match neither set — strictly worse
/// than the unstemmed symmetric baseline. Non-Latin scripts (CJK, Arabic,
/// Cyrillic, Turkish…) always count as divergent, because the English Snowball
/// fallback in [`make_stemmer`] would corrupt them.
///
/// Single source of this decision. Every consumer of the keyword kernel that
/// intersects a résumé against a posting MUST route through it, or two surfaces
/// scoring the same pair will disagree — see `commands::match_resume::score_one`,
/// `documents::evidence::rank_bullets` (the trim panel + evidence extraction),
/// and `validate::content::Analysis` (the quality report).
pub fn languages_align(job_text: &str, resume_locale: &str) -> bool {
    match detect(job_text).map(|i| i.lang()) {
        Some(Lang::Deu) => resume_locale.starts_with("de"),
        Some(Lang::Fra) => resume_locale.starts_with("fr"),
        Some(Lang::Spa) => resume_locale.starts_with("es"),
        Some(Lang::Ita) => resume_locale.starts_with("it"),
        Some(Lang::Por) => resume_locale.starts_with("pt"),
        Some(Lang::Nld) => resume_locale.starts_with("nl"),
        // Scripts the English Snowball stemmer cannot handle: always divergent.
        Some(
            Lang::Cmn
            | Lang::Jpn
            | Lang::Kor
            | Lang::Vie
            | Lang::Tha
            | Lang::Ara
            | Lang::Heb
            | Lang::Hin
            | Lang::Ben
            | Lang::Tur
            | Lang::Ukr
            | Lang::Rus,
        ) => false,
        // English is the default Snowball stemmer; any other unrecognised
        // language aligns only when the résumé locale says English.
        _ => resume_locale.starts_with("en"),
    }
}

/// The ISO-639-1 tag this crate has a use for `lang`, or `None` for every
/// other whatlang-recognised language (Polish, Swedish, Czech, Romanian,
/// Greek, …). The one 19-language table [`detect_locale_tag`] (stemmer
/// selection, unconditional) and [`detected_language`] (language IDENTITY,
/// confidence-gated) both read from, so the two answers cannot silently
/// diverge into two different sets of "languages this crate knows".
pub(super) fn locale_tag_of(lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::Eng => Some("en"),
        Lang::Deu => Some("de"),
        Lang::Fra => Some("fr"),
        Lang::Spa => Some("es"),
        Lang::Ita => Some("it"),
        Lang::Por => Some("pt"),
        Lang::Nld => Some("nl"),
        Lang::Cmn => Some("zh"),
        Lang::Jpn => Some("ja"),
        Lang::Kor => Some("ko"),
        Lang::Vie => Some("vi"),
        Lang::Tha => Some("th"),
        Lang::Ara => Some("ar"),
        Lang::Heb => Some("he"),
        Lang::Hin => Some("hi"),
        Lang::Ben => Some("bn"),
        Lang::Tur => Some("tr"),
        Lang::Ukr => Some("uk"),
        Lang::Rus => Some("ru"),
        _ => None,
    }
}

/// Best-effort language tag for text whose locale is not stored — the shape
/// [`languages_align`] expects on its `resume_locale` side.
///
/// Needed because a résumé being scored straight out of the generator has no
/// persisted `locale` the way a `DocumentRecord` does. Non-Latin languages are
/// mapped to their own tags rather than collapsing into the `"en"` fallback:
/// a Japanese résumé that answered `"en"` here would align with an English
/// posting and get stemmed by the English Snowball stemmer.
///
/// Deliberately **unconditional** on `whatlang`'s confidence, unlike
/// [`detected_language`]: this picks a STEMMER, and a low-confidence German
/// read still beats stemming German prose with the English algorithm — some
/// stemmer must be chosen, and English is not a privileged default here the
/// way it is for the IDENTITY question. Shares [`locale_tag_of`]'s table with
/// `detected_language` so the two functions can only ever differ in POLICY
/// (gated vs. not), never in which languages they recognise.
pub fn detect_locale_tag(text: &str) -> &'static str {
    detect(text)
        .and_then(|info| locale_tag_of(info.lang()))
        .unwrap_or("en")
}

/// Confidence `whatlang` must clear before its answer is trusted as this
/// text's language, in [`detected_language`]. Below this, `whatlang` is
/// guessing, not reading — documentation of the bar `whatlang::Info::is_reliable`
/// uses internally, **not** a copy compared against independently:
/// [`detected_language`] calls `info.is_reliable()` directly, so this crate's
/// "confident" and the library's own cannot drift apart even at the boundary
/// (`is_reliable` is `confidence() > 0.9`, strictly greater — a value of
/// exactly `0.9` is NOT reliable, a distinction a hand-rolled `< 0.9`
/// comparison against this const got backwards; see
/// `test::whatlang_reliability_boundary_is_strictly_greater_than_0_9` for the
/// boundary pinned directly against the library).
///
/// Calibrated against this crate's own fixtures, not picked in the abstract:
/// every full résumé, job ad and drifted-résumé-SECTION in the
/// `validate::content` fixture corpus reads at confidence 1.0; the two
/// documented false-positive shapes — a keyword-soup job ad ("Terraform AWS
/// PostgreSQL Kubernetes platform engineer") and a short certifications block
/// — read at 0.08 and 0.13. There is real air between the two clusters at 0.9.
pub const MIN_DETECTION_CONFIDENCE: f64 = 0.9;

/// The ISO-639-1 tag for `text`'s detected language, or `None` when the
/// detector's answer has no tag here — either because `whatlang` was not
/// confident enough ([`MIN_DETECTION_CONFIDENCE`]), or because it read a
/// language [`locale_tag_of`] does not cover. The language-IDENTITY question,
/// kept separate from [`languages_align`]'s stemmer-compatibility question.
///
/// `None` rather than an `"en"` guess for anything whatlang recognises outside
/// [`locale_tag_of`]'s table (Polish, Swedish, Czech, Romanian, Greek, …): an
/// identity answer that silently says "this is English" for a language it
/// never looked at would produce a false Critical the moment a caller compares
/// it against any target other than `"en"`. And `None` rather than a guess
/// below the confidence bar, for the same reason `validate::content` states
/// everywhere else: where a check cannot be made reliably it goes quiet rather
/// than guessing.
///
/// Every caller of this function is either an ACCUSATION (a document read as
/// the wrong language) or an ENABLING/CORROBORATING check (is the target
/// language itself credible). The confidence gate protects the first
/// direction — a low-confidence `None` can never manufacture a false
/// accusation — and only ever makes the second one quieter. A mis-calibrated
/// [`MIN_DETECTION_CONFIDENCE`] can make this function under-fire; it cannot
/// make it lie.
pub fn detected_language(text: &str) -> Option<&'static str> {
    let info = detect(text)?;
    if !info.is_reliable() {
        return None;
    }
    locale_tag_of(info.lang())
}
