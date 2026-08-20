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

/// Curated function words / generic job-ad filler for the six Snowball
/// languages [`make_stemmer`] stems for, applied by [`keywords_normalized_list`]
/// in place of the English-only [`STOPWORDS`] when the SAME text detects as
/// that language. Fixes the defect where a German posting's coverage
/// denominator was inflated by German function words and adjectives
/// (`abgeschlossenes`, `abgestimmt`, `abseits`, `abhängig`, …) that
/// `STOPWORDS` never covered, tanking every non-English match score.
///
/// Curated, not exhaustive: function words plus filler unambiguously
/// comparable to `STOPWORDS`'s own "Job-ad filler" section (`erfahrung` /
/// "experience", `kandidat` / "candidate", …). A word that is arguably a real
/// skill signal is deliberately left OUT — e.g. `agil`/`agilen` ("agile") is
/// a real methodology keyword, not filler, so it is absent here even though
/// it surfaced in the same buggy recommendations list as the words above.
///
/// **Deliberately separate lists from two other consts with overlapping
/// content, not "unified" with either — do not merge them:**
/// - `validate::content::language`'s `FUNCTION_WORDS_DE` (+ the other five)
///   answers a language-IDENTITY question for a DIFFERENT set of curated
///   languages, at a different correctness bar (see that module's doc).
/// - `documents::evidence::mod`'s `FUNCTION_WORDS_DE` is a DISPLAY-only
///   skill-claim filter, explicitly NOT formula-version-pinned (see its doc
///   comment, "do not move these into `documents::keywords::STOPWORDS`").
///   These lists ARE formula-version-pinned (`commands::match_resume::
///   MATCH_FORMULA_VERSION`) because they change every document's keyword
///   set — that is the whole point of the fix.
const STOPWORDS_DE: &[&str] = &[
    // Conjunctions / subordinators.
    "dass",
    "wenn",
    "weil",
    "denn",
    "doch",
    "noch",
    "aber",
    "oder",
    "auch",
    "sowie",
    "sowohl",
    "sondern",
    "damit",
    "sodass",
    // Determiners / pronouns.
    "eine",
    "einem",
    "einen",
    "einer",
    "eines",
    "diese",
    "dieser",
    "dieses",
    "diesem",
    "diesen",
    "jeder",
    "jede",
    "jedem",
    "jeden",
    "jedes",
    "unser",
    "unsere",
    "unserem",
    "unseren",
    "unserer",
    "ihre",
    "ihrem",
    "ihren",
    "ihrer",
    "ihnen",
    "mein",
    "meine",
    "dein",
    "deine",
    "sein",
    "seine",
    "selbst",
    // Adverbs.
    "sehr",
    "schon",
    "immer",
    "mehr",
    "etwa",
    "ganz",
    "eher",
    "dabei",
    "dann",
    // Prepositions (für is 4 BYTES despite reading 3 chars — see w.len()'s
    // byte-length note on the kernel filter).
    "für",
    "über",
    "unter",
    "durch",
    "gegen",
    "ohne",
    "nach",
    "seit",
    "beim",
    "hinter",
    "zwischen",
    "während",
    "innerhalb",
    // Modal / auxiliary verbs.
    "haben",
    "hatte",
    "hatten",
    "wird",
    "werden",
    "wurde",
    "wurden",
    "kann",
    "können",
    "muss",
    "müssen",
    "sind",
    "sollte",
    "sollten",
    "würde",
    "würden",
    "könnte",
    "könnten",
    // Job-ad filler, including the exact reported defect words.
    "abgeschlossenes",
    "abgeschlossene",
    "abgeschlossener",
    "abgeschlossenen",
    "abgestimmt",
    "abseits",
    "abwechslungsreiche",
    "abwechslungsreichen",
    "abhängig",
    "erfahrung",
    "erfahrene",
    "erfahrener",
    "erfahrungen",
    "kenntnisse",
    "qualifikationen",
    "anforderungen",
    "aufgaben",
    "voraussetzungen",
    "wünschenswert",
    "verantwortlich",
    "bereits",
    "bieten",
    "gerne",
    "gute",
    "guten",
    "idealerweise",
    "suchen",
    "unternehmen",
    "position",
    "kandidat",
    "kandidatin",
    "vorteile",
    "bonus",
];
const STOPWORDS_FR: &[&str] = &[
    "avec",
    "dans",
    "pour",
    "sans",
    "sous",
    "chez",
    "vers",
    "cette",
    "leur",
    "leurs",
    "notre",
    "votre",
    "vos",
    "nous",
    "vous",
    "elle",
    "elles",
    "tout",
    "tous",
    "toute",
    "toutes",
    "être",
    "avoir",
    "sont",
    "était",
    "étaient",
    "sera",
    "seront",
    "avez",
    "avons",
    "peut",
    "peuvent",
    "doit",
    "doivent",
    "aussi",
    "ainsi",
    "alors",
    "comme",
    "donc",
    "mais",
    "quand",
    "dont",
    "quel",
    "quelle",
    "quels",
    "quelles",
    "entre",
    "après",
    "avant",
    "pendant",
    "depuis",
    "jusqu",
    "plus",
    "moins",
    "très",
    "bien",
    "tant",
    "même",
    // Job-ad filler.
    "expérience",
    "expériences",
    "compétences",
    "connaissances",
    "capacité",
    "capacités",
    "poste",
    "candidat",
    "candidate",
    "entreprise",
    "société",
    "responsabilités",
    "exigences",
    "qualifications",
    "avantages",
    "souhaité",
    "souhaitée",
    "requis",
    "requise",
    "recherche",
    "recherchons",
    "équipe",
];
const STOPWORDS_ES: &[&str] = &[
    "para",
    "como",
    "pero",
    "esta",
    "este",
    "estos",
    "estas",
    "sus",
    "también",
    "cuando",
    "donde",
    "porque",
    "aunque",
    "entre",
    "sobre",
    "desde",
    "hasta",
    "sin",
    "más",
    "menos",
    "todo",
    "toda",
    "todos",
    "todas",
    "otro",
    "otra",
    "otros",
    "otras",
    "cada",
    "sido",
    "está",
    "están",
    "será",
    "serán",
    "puede",
    "pueden",
    "debe",
    "deben",
    "tiene",
    "tienen",
    "haber",
    "hacer",
    // Job-ad filler.
    "experiencia",
    "conocimientos",
    "habilidades",
    "capacidad",
    "puesto",
    "candidato",
    "candidata",
    "empresa",
    "responsabilidades",
    "requisitos",
    "requisito",
    "calificaciones",
    "beneficios",
    "deseable",
    "buscamos",
    "equipo",
];
const STOPWORDS_IT: &[&str] = &[
    "questo",
    "questa",
    "questi",
    "queste",
    "loro",
    "nostro",
    "nostra",
    "nostri",
    "nostre",
    "vostro",
    "vostra",
    "essere",
    "avere",
    "sono",
    "stato",
    "stata",
    "sarà",
    "saranno",
    "puoi",
    "può",
    "possono",
    "deve",
    "devono",
    "anche",
    "come",
    "però",
    "quando",
    "dove",
    "perché",
    "sebbene",
    "sopra",
    "sotto",
    "senza",
    "dopo",
    "prima",
    "durante",
    "molto",
    "poco",
    "tutto",
    "tutta",
    "tutti",
    "tutte",
    "altro",
    "altra",
    "altri",
    "altre",
    // Job-ad filler.
    "esperienza",
    "esperienze",
    "competenze",
    "conoscenze",
    "capacità",
    "posizione",
    "candidato",
    "candidata",
    "azienda",
    "responsabilità",
    "requisiti",
    "qualifiche",
    "vantaggi",
    "desiderabile",
    "cerchiamo",
    "squadra",
];
const STOPWORDS_PT: &[&str] = &[
    "para",
    "como",
    "mais",
    "muito",
    "esta",
    "este",
    "estes",
    "estas",
    "seus",
    "suas",
    "nosso",
    "nossa",
    "nossos",
    "nossas",
    "quando",
    "onde",
    "porque",
    "embora",
    "entre",
    "sobre",
    "desde",
    "após",
    "antes",
    "durante",
    "todo",
    "toda",
    "todos",
    "todas",
    "outro",
    "outra",
    "outros",
    "outras",
    "cada",
    "sido",
    "está",
    "estão",
    "será",
    "serão",
    "pode",
    "podem",
    "deve",
    "devem",
    "têm",
    // Job-ad filler.
    "experiência",
    "experiências",
    "conhecimentos",
    "habilidades",
    "capacidade",
    "posição",
    "candidato",
    "candidata",
    "empresa",
    "responsabilidades",
    "requisitos",
    "qualificações",
    "benefícios",
    "desejável",
    "procuramos",
    "equipe",
    "equipa",
];
const STOPWORDS_NL: &[&str] = &[
    "deze",
    "onze",
    "jouw",
    "jullie",
    "zijn",
    "haar",
    "wordt",
    "worden",
    "werd",
    "werden",
    "kunnen",
    "moet",
    "moeten",
    "heeft",
    "hebben",
    "hadden",
    "ook",
    "maar",
    "wanneer",
    "waar",
    "omdat",
    "hoewel",
    "tussen",
    "boven",
    "onder",
    "zonder",
    "voor",
    "over",
    "sinds",
    "tijdens",
    "veel",
    "weinig",
    "alle",
    "andere",
    "elke",
    // Job-ad filler.
    "ervaring",
    "kennis",
    "vaardigheden",
    "vaardigheid",
    "functie",
    "kandidaat",
    "bedrijf",
    "verantwoordelijkheden",
    "vereisten",
    "kwalificaties",
    "voordelen",
    "gewenst",
    "zoeken",
    "team",
];

/// The Snowball [`Algorithm`] AND stopword list for the language detected in
/// `text` — single detect()+mapping call site for both [`make_stemmer`] and
/// [`keywords_normalized_list`]'s stopword choice, so the two answers for the
/// SAME text can never independently drift the way `STOPWORDS` being
/// English-only silently did.
fn language_profile(text: &str) -> (Algorithm, &'static [&'static str]) {
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
fn stopwords_for_lang(lang: &str) -> &'static [&'static str] {
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
fn locale_tag_of(lang: Lang) -> Option<&'static str> {
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

/// Normalize text to a language-agnostic keyword set: lowercase,
/// synonym-normalized, filtered - but NOT stemmed. Tokens shorter than 4 chars
/// are dropped unless they are in SHORT_TECH_TERMS; stopwords are excluded.
/// The slash is kept in tokenization so ci/cd survives as a single token.
///
/// Store this in the DB - apply apply_stemmer at match time to stay
/// language-agnostic (the stemmer language is detected from the JD, not the
/// resume, so caching a pre-stemmed set would bake in the wrong language).
///
/// A thin `collect()` over [`keywords_normalized_list`] so the set form and the
/// occurrence-counting list form can never drift apart.
pub fn keywords_normalized(text: &str) -> HashSet<String> {
    keywords_normalized_list(text).into_iter().collect()
}

/// Duplicate-preserving, document-ordered form of [`keywords_normalized`] — the
/// SAME tokenizer, synonym collapse and filter, returning every surviving token
/// instead of deduplicating them.
///
/// Exists for the consumers that must count *repeats* rather than membership
/// (the ATS keyword-density check in `validate::content::ats`). Deliberately the
/// single implementation of the pipeline, with `keywords_normalized` delegating
/// to it: a second tokenizer written "just to count" is exactly the fork the
/// keyword kernel exists to prevent.
pub fn keywords_normalized_list(text: &str) -> Vec<String> {
    // Same text, same detection call site `make_stemmer` uses (via
    // `language_profile`) — the stopword language can never disagree with the
    // stemmer language for this call. Correct for a caller that tokenizes ONE
    // self-contained document (a whole résumé or JD); see
    // [`keywords_normalized_list_for_lang`] for the short-fragment case where
    // this per-call detection is NOT safe.
    normalize_list_with_stopwords(text, language_profile(text).1)
}

/// The [`keywords_normalized_list`] pipeline, but the stopword LANGUAGE is
/// pinned to an explicit ISO-639-1 tag rather than re-detected from `text`.
///
/// For a caller that already resolved ONE language decision for a whole
/// document (e.g. `validate::content::Analysis::lang`, or
/// `DocumentTokens`/`Analysis`'s stemmer) and then tokenizes many SHORT
/// per-line, per-title or per-bullet fragments of it: `whatlang` reading an
/// isolated short line in isolation is unreliable ("Kenntnisse in Rust,
/// Python, Kubernetes, Terraform und Kafka" reads as Estonian at confidence
/// 0.23, not German), and a per-call re-detection can silently pick a
/// DIFFERENT stopword list than the one the whole document resolved to. A
/// filler word filtered out of the document-level set (and so absent from its
/// stem→readable [`display_forms`] map) would then survive un-filtered from
/// the line-level call and leak out as a raw, unreadable stem instead of
/// being suppressed. Mirrors [`keywords_normalized_list`] exactly — same
/// shared tokenizer, only the stopword SOURCE differs — so the two can never
/// diverge on tokenization itself, only on which language's filler they drop.
pub fn keywords_normalized_list_for_lang(text: &str, lang: &str) -> Vec<String> {
    normalize_list_with_stopwords(text, stopwords_for_lang(lang))
}

fn normalize_list_with_stopwords(text: &str, stopwords: &[&str]) -> Vec<String> {
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
                && !stopwords.contains(&s)
                // Pure-numeric tokens (postcodes, bare years) carry no keyword
                // signal on either side. Mixed alphanumeric tech tokens (c4, s3,
                // oauth2, es2015) are untouched - at least one char isn't a digit.
                && !s.chars().all(|c| c.is_ascii_digit())
        })
        .collect()
}

/// [`keywords_normalized`], with the stopword language pinned explicitly —
/// see [`keywords_normalized_list_for_lang`].
pub fn keywords_normalized_for_lang(text: &str, lang: &str) -> HashSet<String> {
    keywords_normalized_list_for_lang(text, lang)
        .into_iter()
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

/// [`keywords`], with the stopword language pinned explicitly — see
/// [`keywords_normalized_list_for_lang`]. `stemmer` is still whatever the
/// caller already built (typically also from the SAME resolved language, via
/// [`make_stemmer`] on the whole document); this only changes which stopword
/// list filters `text` before stemming.
pub fn keywords_for_lang(text: &str, lang: &str, stemmer: &Stemmer) -> HashSet<String> {
    apply_stemmer(keywords_normalized_for_lang(text, lang), stemmer)
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
    display_forms_from(keywords_normalized(job_text), stemmer)
}

/// [`display_forms`], with the stopword language pinned explicitly — see
/// [`keywords_normalized_list_for_lang`]. Needed alongside
/// [`keywords_for_lang`]/[`keywords_normalized_for_lang`]: a caller that
/// tokenizes under an explicit lang MUST build its display map the same way,
/// or a word the explicit-lang tokenizer keeps (because the auto-detected
/// language would have dropped it, or vice versa) gets a display entry that
/// doesn't match what was actually tokenized.
pub fn display_forms_for_lang(
    job_text: &str,
    lang: &str,
    stemmer: &Stemmer,
) -> HashMap<String, String> {
    display_forms_from(keywords_normalized_for_lang(job_text, lang), stemmer)
}

fn display_forms_from(normalized: HashSet<String>, stemmer: &Stemmer) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    // Iterate a sorted Vec, not the HashSet, so the `or_insert` winner for two
    // tokens sharing a stem is deterministic across runs.
    let mut tokens: Vec<_> = normalized.into_iter().collect();
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
mod test;
