//! Function words the help LEXICAL arm drops from a question before the
//! OR-join (`retrieval::lexical::QueryMode::Any`).
//!
//! **Why these are separate from `documents::keywords::STOPWORDS`.** That
//! family (English + six more) is curated behind the keyword pipeline's
//! `len > 3` filter, so it contains no entry under 3 characters by
//! construction — and the tokens that actually inflate an OR query are
//! exactly the short ones (en `how do my as is to of in it`, de
//! `wie was wo ich und der die das`). Reusing it was MEASURED against the
//! real corpus (`tests/help_retrieval.rs`) and the entire top-2 gain came
//! from ONE entry, the job-ad filler word `work` — which is help-domain
//! CONTENT here ("so the AI features work", "How does the search box
//! work?"): removing just that word put the case straight back to rank 3,
//! and no other entry in the list moved a single case. The German list moved
//! nothing at all. Those lists are also pinned to
//! `commands::match_resume::MATCH_FORMULA_VERSION`, so editing them for a
//! help-search reason would re-score every stored document.
//!
//! **Interrogatives, articles, pronouns, prepositions, conjunctions and
//! auxiliaries ONLY.** No content verbs or nouns, and deliberately no
//! quantifiers of totality (`all`, `alles`) or negations: over-filtering is
//! how this goes wrong, and it goes wrong SILENTLY. The measured German
//! counter-example is pinned as a test case — adding `alles` turns "Wie setze
//! ich alles zurück?" from a hit into a MISS, because the entry's own title
//! says "alles zurücksetzen" and FTS5 does no decompounding, so `zurück`
//! never matches `zurücksetzen`. German compounding means fewer tokens
//! survive a question and each one carries more, which is why the two lists
//! are hand-curated per language rather than machine-derived from one.

/// The locale prefix cap. A locale is caller input (a `help_search` is
/// reachable from the agent CLI with a hand-written body), so an unbounded
/// string never reaches the lowercasing below. Anything longer, or carrying
/// anything but ASCII letters and `-`, is treated as an UNKNOWN locale rather
/// than an error: the drop list is a ranking nicety, and refusing a whole
/// search over it would be the worse failure.
const LOCALE_MAX_CHARS: usize = 16;

/// English function words. Every entry is folded the way
/// `retrieval::lexical` folds a query token (lowercase, edge punctuation
/// trimmed), so entries here are lowercase and unpunctuated.
///
/// Measured, not guessed: with this list the committed English eval moves
/// from 17 to 18 of 18 cases inside the narrow top-2 limit while every case
/// keeps at least two contenders (`tests/help_retrieval.rs` prints both
/// columns). The single-character entries are already dropped by
/// `retrieval::lexical::ANY_MIN_TOKEN_CHARS` before this list is consulted;
/// they are kept for completeness so the list reads as the vocabulary it is.
pub(crate) const HELP_STOPWORDS_EN: &[&str] = &[
    // Articles + determiners (not quantifiers — see the module doc).
    "a", "an", "the", //
    // Pronouns.
    "i", "me", "my", "mine", "myself", "we", "us", "our", "ours", "you", "your", "yours", "he",
    "him", "his", "she", "her", "hers", "it", "its", "they", "them", "their", "theirs", "this",
    "that", "these", "those", //
    // Interrogatives.
    "who", "whom", "whose", "what", "which", "when", "where", "why", "how", //
    // Auxiliaries and copulas.
    "am", "are", "be", "been", "being", "is", "was", "were", "do", "does", "did", "doing", "have",
    "has", "had", "having", "can", "could", "may", "might", "must", "shall", "should", "will",
    "would", //
    // Prepositions.
    "about", "after", "at", "before", "between", "by", "during", "for", "from", "in", "into", "of",
    "off", "on", "onto", "out", "over", "through", "to", "under", "up", "with", "within",
    "without", //
    // Conjunctions.
    "and", "as", "because", "but", "if", "or", "so", "than", "then", "while",
];

/// German function words — same categories, same folding, same exclusions.
/// `alles`/`alle` are absent DELIBERATELY (module doc): they are the measured
/// counter-example, and `tests/help_retrieval.rs` carries the case that goes
/// red if they are ever added.
pub(crate) const HELP_STOPWORDS_DE: &[&str] = &[
    // Articles.
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer",
    "eines", //
    // Pronouns.
    "ich", "mich", "mir", "mein", "meine", "meinen", "meinem", "meiner", "meines", "du", "dich",
    "dir", "dein", "deine", "er", "ihn", "ihm", "sie", "ihr", "ihre", "ihren", "es", "wir", "uns",
    "unser", "unsere", "man", "sich", "dies", "diese", "dieser", "dieses", "diesen",
    "diesem", //
    // Interrogatives.
    "wie", "was", "wo", "wann", "warum", "wieso", "weshalb", "welche", "welcher", "welches",
    "welchen", "welchem", "wer", "wen", "wem", "wessen", "woher", "wohin", //
    // Auxiliaries and modals.
    "bin", "bist", "ist", "sind", "seid", "war", "waren", "sein", "habe", "hast", "hat", "haben",
    "hatte", "hatten", "kann", "kannst", "können", "könnte", "muss", "müssen", "soll", "sollen",
    "will", "wollen", "werde", "wird", "werden", "wurde", "wurden", "möchte", "möchten", //
    // Prepositions.
    "am", "an", "auf", "aus", "bei", "beim", "bis", "durch", "für", "gegen", "im", "in", "ins",
    "mit", "nach", "ohne", "seit", "um", "unter", "über", "von", "vom", "vor", "zu", "zum", "zur",
    "zwischen", //
    // Conjunctions and particles.
    "aber", "als", "auch", "damit", "dass", "denn", "doch", "noch", "nur", "ob", "oder", "sondern",
    "sowie", "und", "weil", "wenn",
];

/// The drop list for a caller-declared locale, or an EMPTY slice for one this
/// module has no hand-curated list for.
///
/// Empty, never English: a French or Japanese corpus filtered by English
/// function words would drop nothing useful and could drop a real term
/// (`in`, `an`, `es` are content in other languages), and silently ranking a
/// foreign corpus by an English assumption is exactly the failure the
/// module doc's German counter-example describes.
///
/// **An OMITTED locale lands here too, as the empty string.**
/// `HelpSearchRequestSchema.locale` is optional and `help_search` passes
/// `""` for `None` rather than `"en"`: a caller that never said which
/// language its entries are in has not said English, and the safe answer to
/// "unknown" is the same one every other unknown tag gets — drop nothing.
///
/// Normalised to the primary subtag, lowercased (`de-AT` → `de`, `EN` →
/// `en`), because the renderer sends `i18n.language`, which carries a region
/// on some installs.
pub(crate) fn stopwords_for_locale(locale: &str) -> &'static [&'static str] {
    match normalize_locale(locale).as_deref() {
        Some("en") => HELP_STOPWORDS_EN,
        Some("de") => HELP_STOPWORDS_DE,
        _ => &[],
    }
}

/// Primary subtag, lowercased, or `None` for anything that is not a plausible
/// BCP-47 tag. The length/charset check runs BEFORE the allocation, so a
/// caller-supplied megabyte "locale" costs one comparison rather than a
/// lowercase copy of itself.
fn normalize_locale(locale: &str) -> Option<String> {
    if locale.len() > LOCALE_MAX_CHARS
        || !locale.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
    {
        return None;
    }
    let primary = locale.split('-').next().unwrap_or_default();
    if primary.is_empty() {
        return None;
    }
    Some(primary.to_ascii_lowercase())
}
