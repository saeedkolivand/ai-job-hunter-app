//! The curated German and Dutch stopword lists applied by `keywords_normalized_list` in place of
//! the English-only `STOPWORDS` when the text detects as that language.
//!
//! Split out of `documents/keywords.rs` (R8's hard LOC cap); everything here moved verbatim.
//! The doc on [`STOPWORDS_DE`] is the contract for EVERY per-language list.

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
pub(super) const STOPWORDS_DE: &[&str] = &[
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

pub(super) const STOPWORDS_NL: &[&str] = &[
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
