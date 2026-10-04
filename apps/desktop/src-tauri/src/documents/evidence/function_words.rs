//! The display-only function-word filter for the skills split.
//!
//! Split out of `evidence/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. Deliberately LOCAL to the evidence module — see [`FUNCTION_WORDS_DE`]'s
//! doc for why it must never be merged into the scoring kernel's `STOPWORDS`.

/// Function words and posting filler that are not skills, per language.
///
/// **Deliberately LOCAL to this module — do not move these into
/// `documents::keywords::STOPWORDS`.** That list feeds the scoring kernel and is
/// pinned to the match-score formula version: adding a word to it changes every
/// document's keyword set and silently invalidates every cached match score.
/// `skills_present`/`skills_absent` are a NEW, display-only surface, so the
/// filter belongs here, where it can only affect what the user reads.
///
/// `STOPWORDS` itself is English-only, which is why a German posting fills the
/// gap list with "unsere", "hinter" and "bereits" instead of skills. Entries are
/// the *unstemmed, normalized* forms (what [`display_forms`] yields), because
/// the split happens on stems when the languages align.
///
/// **Not the same primitive as `validate::content::language`'s
/// `FUNCTION_WORDS_DE`/`function_words_for`, despite the identical name and
/// overlapping content — do not "unify" them.** That module asks a language-
/// IDENTITY question (does this text carry positive evidence of being
/// written in a specific OTHER language, so a confident-but-wrong `whatlang`
/// read is not this crate's only witness) and answers it for seven curated
/// languages; this one asks a SKILL-CLAIM question (which tokens on a skills
/// line are filler rather than a claimed skill) and, as the doc above
/// explains, is deliberately curated for German only. A word missing here is
/// safe (a display-only gap list stays merely noisy); a word wrongly
/// PRESENT there is not (it manufactures a language accusation) — the two
/// lists answer to different correctness bars for that reason, and merging
/// them would let a change tuned for one silently regress the other.
const FUNCTION_WORDS_DE: &[&str] = &[
    // Determiners, pronouns and prepositions that survive the ≥4-char filter.
    "aber",
    "auch",
    "beim",
    "dabei",
    "damit",
    "dann",
    "dass",
    "dein",
    "deine",
    "diese",
    "diesem",
    "diesen",
    "dieser",
    "dieses",
    "durch",
    "eine",
    "einem",
    "einen",
    "einer",
    "eines",
    // Four BYTES, so the kernel's `w.len() > 3` filter never drops it however
    // short it reads — and it is the commonest preposition in the language.
    "für",
    "hinter",
    "ihre",
    "ihrem",
    "ihren",
    "ihnen",
    "jede",
    "jeden",
    "mehr",
    "nach",
    "noch",
    "oder",
    "ohne",
    "schon",
    "sehr",
    "selbst",
    "sowie",
    "unser",
    "unsere",
    "unserem",
    "unseren",
    "unter",
    "wenn",
    "zwischen", // Copulas and modals.
    "haben",
    "hast",
    "hatte",
    "kann",
    "können",
    "muss",
    "müssen",
    "sein",
    "seine",
    "sind",
    "sollte",
    "sollten",
    "werden",
    "wird",
    "wurde",
    "wurden", // Posting filler — the German
    // half of the English filler `STOPWORDS` already drops ("experience",
    // "skills", "requirements", …).
    "anforderungen",
    "aufgaben",
    "bereits",
    "bieten",
    "erfahrung",
    "erfahrene",
    "erfahrungen",
    "gerne",
    "gute",
    "guten",
    "idealerweise",
    "kenntnisse",
    "profil",
    "suchen",
    // "Verantwortlich für …" opens half the bullets in a German résumé. It
    // names no skill, and counted as a keyword it read as stuffing.
    "verantwortlich",
    "voraussetzungen",
    "wünschenswert",
];

/// The function-word list for `lang`, empty for languages with no list yet
/// (English is already covered by the kernel's own `STOPWORDS`).
///
/// `pub(crate)` for `validate::content::ats`, whose keyword-density check counts
/// tokens through the same English-only `STOPWORDS` plus a BYTE-length filter —
/// so `durch`, `werden` and `wurde` all counted as keywords and ordinary German
/// prose was accused of stuffing. One list, both surfaces: a word that is not a
/// skill in the gap list is not a stuffed keyword either.
///
/// **An empty list means "no filter", not "nothing to filter"** — see
/// [`has_curated_function_words`], which is what a caller must ask before
/// drawing a conclusion from a count.
pub(crate) fn function_words(lang: &str) -> &'static [&'static str] {
    match lang {
        "de" => FUNCTION_WORDS_DE,
        _ => &[],
    }
}

/// Whether `lang`'s function words are actually known to this crate.
///
/// English counts: the kernel's own `STOPWORDS` is its curation, which is why
/// [`function_words`] returns an empty slice for `en` without that meaning
/// "unfiltered". Every other language returns an empty slice because nobody has
/// written the list yet — the two cases are indistinguishable at the call site,
/// and that is the whole point of this helper.
///
/// The rule it exists to enforce, the same one the rest of this module lives by:
/// **never accuse without evidence.** A ratio or a ceiling computed over
/// unfiltered French, Spanish, Italian, Dutch or Portuguese prose counts `pour`,
/// `para`, `nella`, `worden` and `para` as keywords, so ordinary writing reads as
/// stuffing. A caller whose conclusion depends on function words having been
/// removed must go quiet here rather than report a number it cannot stand
/// behind. Adding a language to [`function_words`] is what re-enables it —
/// deliberately one edit, in one place.
///
/// `false` for `"es"` here does NOT mean this crate has no Spanish function
/// words anywhere — `validate::content::language::function_words_for("es")`
/// returns 61 of them, curated for a DIFFERENT question (see
/// [`FUNCTION_WORDS_DE`]'s doc for why the two are deliberately separate
/// primitives, not the same list read from two places).
pub(crate) fn has_curated_function_words(lang: &str) -> bool {
    matches!(lang, "en" | "de")
}
