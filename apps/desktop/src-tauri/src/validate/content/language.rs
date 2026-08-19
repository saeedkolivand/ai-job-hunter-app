//! Whether the generated document — or one of its sections — reads as the
//! target language. Split out of `mod.rs` to keep it under R8's line cap
//! (`docs/architecture-rules.md`).
//!
//! ## Known limit
//!
//! Two detectors decide and check the same question. The renderer picks the
//! target language with **franc**; this module validates with **whatlang**
//! (via [`detected_language`]). [`target_is_corroborated`] therefore really
//! asks "does whatlang agree with franc about this ad" — when the two
//! detectors disagree, the guard goes quiet. Consistent with this module's
//! posture everywhere else (a check that cannot be made reliably goes quiet
//! rather than guesses), but it is a real limit, not a future bug report.
//!
//! ## Mitigated limit: whatlang's confidence is a MARGIN, not a probability
//!
//! `is_language_mismatch` used to trust a confident [`detected_language`]
//! read of the GENERATED text on its own. That read can be confidently
//! WRONG: `whatlang`'s n-gram model needs closed-class function words (the
//! articles, prepositions, pronouns, copulas any real sentence is full of) to
//! tell languages apart, and an ordinary noun-phrase-heavy block — a skills
//! line, a terse CV — starves it of them. Measured: a truthful ENGLISH
//! noun-phrase block reads as French at `confidence() == 1.0`,
//! `is_reliable() == true`. `is_language_mismatch` now also requires
//! [`distinctive_evidence_confirms`] — real, PAIRWISE function-word evidence
//! that the whatlang-named language is better supported in the text than the
//! target itself is — before a confident Latin-script read becomes an
//! accusation. See that function's doc for why the check is comparative
//! (found vs. target in the SAME text) rather than a single global pool: a
//! global pool was measured to leave Spanish/Portuguese too thin to evidence
//! themselves (a true-positive regression), and neither a pairwise-only nor a
//! higher-threshold-only fix survives a genuinely English document that
//! merely NAMES a German institution or French award. See
//! [`pairwise_evidence_count`] and [`distinctive_evidence_confirms`] for the
//! measurements behind both halves.

use std::collections::HashSet;

use super::{
    issue, significant_chars, Analysis, ContentIssue, Section, SectionKind, Severity,
    CONTENT_LANGUAGE_MISMATCH,
};
use crate::documents::keywords::detected_language;

/// Below this many non-whitespace characters, `whatlang` guesses. A Critical
/// language mismatch on a two-line draft would be a false accusation, so the
/// check goes quiet instead.
pub(super) const MIN_CHARS_FOR_LANGUAGE_CHECK: usize = 120;

/// Function words per Latin-script curated language — the raw vocabulary
/// [`pairwise_evidence_count`] draws on. Kept honest per language (the words
/// a fluent speaker would actually list) rather than hand-pruned for
/// collisions; pruning is a PAIRWISE, per-comparison decision now (see
/// [`pairwise_evidence_count`]'s doc for why a single global pool — pruning a
/// word the moment it appears in ANY two of the seven lists — was measured to
/// leave Spanish and Portuguese too thin to evidence themselves at all).
const FUNCTION_WORDS_EN: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "so", "as", "of", "in", "on", "at", "to", "by",
    "for", "with", "from", "into", "onto", "about", "over", "under", "between", "through",
    "during", "before", "after", "this", "that", "these", "those", "it", "he", "she", "him", "her",
    "his", "they", "them", "their", "we", "us", "our", "you", "your", "i", "my", "is", "are",
    "was", "were", "be", "been", "being", "have", "has", "had", "do", "does", "did", "will",
    "would", "can", "could", "should", "must", "not", "no", "per",
];
const FUNCTION_WORDS_DE: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines",
    "im", "am", "beim", "vom", "zum", "zur", "und", "oder", "aber", "doch", "denn", "weil", "dass",
    "wenn", "als", "wie", "ob", "in", "an", "auf", "aus", "bei", "mit", "nach", "seit", "von",
    "zu", "für", "durch", "gegen", "ohne", "um", "über", "unter", "zwischen", "hinter", "ich",
    "du", "er", "sie", "es", "wir", "ihr", "mich", "dich", "sich", "uns", "euch", "mein", "dein",
    "sein", "ihre", "unser", "euer", "ist", "sind", "war", "waren", "bin", "bist", "seid", "habe",
    "hast", "hat", "haben", "hatte", "wird", "werden", "kann", "muss", "soll",
];
const FUNCTION_WORDS_FR: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "du", "au", "aux", "et", "ou", "mais", "donc", "car",
    "ni", "que", "si", "quand", "de", "à", "en", "dans", "sur", "sous", "avec", "sans", "pour",
    "par", "chez", "vers", "entre", "depuis", "pendant", "je", "tu", "il", "elle", "nous", "vous",
    "ils", "elles", "me", "te", "se", "lui", "leur", "mon", "ton", "son", "notre", "votre", "est",
    "sont", "était", "étaient", "suis", "es", "sommes", "êtes", "être", "été", "avoir", "ai", "as",
    "avons", "avez", "ont", "avait",
];
const FUNCTION_WORDS_ES: &[&str] = &[
    "el", "la", "los", "las", "un", "una", "unos", "unas", "y", "o", "pero", "porque", "que", "si",
    "aunque", "ni", "de", "en", "a", "con", "por", "para", "sin", "sobre", "entre", "hasta",
    "desde", "hacia", "yo", "tú", "él", "ella", "nosotros", "vosotros", "ellos", "ellas", "me",
    "te", "se", "nos", "mi", "tu", "su", "nuestro", "vuestro", "es", "son", "era", "eran", "soy",
    "eres", "somos", "estar", "está", "están", "ser", "fue", "fueron", "tiene", "tienen", "hay",
];
const FUNCTION_WORDS_IT: &[&str] = &[
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "e", "o", "ma", "però", "che", "se",
    "perché", "né", "di", "a", "da", "in", "con", "su", "per", "tra", "fra", "senza", "dentro",
    "sotto", "sopra", "nei", "nella", "dello", "della", "io", "tu", "lui", "lei", "noi", "voi",
    "loro", "mi", "ti", "si", "ci", "vi", "mio", "tuo", "suo", "nostro", "vostro", "è", "sono",
    "era", "erano", "sei", "siamo", "siete", "essere", "stato", "avere", "ha", "hanno", "ho",
    "hai",
];
const FUNCTION_WORDS_NL: &[&str] = &[
    "de", "het", "een", "en", "of", "maar", "want", "dus", "omdat", "als", "dat", "terwijl", "in",
    "op", "aan", "bij", "met", "naar", "van", "voor", "door", "over", "onder", "tussen", "zonder",
    "na", "ik", "jij", "je", "hij", "zij", "wij", "jullie", "ze", "mij", "jou", "hem", "haar",
    "ons", "mijn", "jouw", "zijn", "hun", "is", "was", "waren", "ben", "bent", "heb", "hebt",
    "heeft", "hebben", "had", "hadden", "wordt", "worden", "kan", "moet",
];
const FUNCTION_WORDS_PT: &[&str] = &[
    "o", "a", "os", "as", "um", "uma", "uns", "umas", "e", "ou", "mas", "porque", "que", "se",
    "embora", "nem", "de", "em", "com", "por", "para", "sem", "sobre", "entre", "até", "desde",
    "eu", "tu", "ele", "ela", "nós", "vós", "eles", "elas", "me", "te", "nos", "meu", "teu", "seu",
    "nosso", "vosso", "é", "são", "era", "eram", "sou", "és", "somos", "estar", "está", "estão",
    "ser", "foi", "foram", "tem", "têm", "há",
];

/// The seven languages [`function_words_for`] curates a real vocabulary for —
/// English plus the SAME six Snowball languages `documents::keywords::make_stemmer`
/// stems for, not the full nineteen [`crate::documents::keywords::locale_tag_of`]
/// recognises.
const CURATED_FUNCTION_WORDS: &[(&str, &[&str])] = &[
    ("en", FUNCTION_WORDS_EN),
    ("de", FUNCTION_WORDS_DE),
    ("fr", FUNCTION_WORDS_FR),
    ("es", FUNCTION_WORDS_ES),
    ("it", FUNCTION_WORDS_IT),
    ("nl", FUNCTION_WORDS_NL),
    ("pt", FUNCTION_WORDS_PT),
];

/// The Latin-script languages `documents::keywords::locale_tag_of` recognises
/// — nine of its nineteen tags, not just the seven [`CURATED_FUNCTION_WORDS`]
/// has a real vocabulary for. Turkish (`tr`) and Vietnamese (`vi`) are Latin
/// script too and share the SAME whatlang blind spot (a confident wrong read
/// on function-word-sparse text) as the curated seven; every other tag reads
/// a genuinely distinct SCRIPT, which whatlang already tells apart from Latin
/// at near-1.0 confidence regardless of function-word density.
///
/// This is [`needs_distinctive_evidence`]'s gate, and it must be the SCRIPT
/// boundary, not [`CURATED_FUNCTION_WORDS`] membership: gating on curation
/// instead of script let a confident `tr`/`vi` whatlang guess skip
/// corroboration entirely and raise a Critical with zero evidence — the
/// opposite of what "uncurated" is supposed to mean everywhere else in this
/// crate. [`function_words_for`] still returns `&[]` for `tr`/`vi` (nobody
/// has written those lists), so a genuine `tr`/`vi` mismatch now goes quiet
/// instead — an accepted miss, same shape as every other uncurated-language
/// miss this module already documents, not a new kind of gap.
const LATIN_SCRIPT_LANGUAGES: &[&str] = &["en", "de", "fr", "es", "it", "pt", "nl", "tr", "vi"];

/// `lang`'s curated function-word vocabulary, or `&[]` when this crate has
/// not written one (every language outside [`CURATED_FUNCTION_WORDS`],
/// including the two extra Latin-script tags [`LATIN_SCRIPT_LANGUAGES`]
/// tracks for the corroboration GATE but not for evidence).
pub(super) fn function_words_for(lang: &str) -> &'static [&'static str] {
    CURATED_FUNCTION_WORDS
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, words)| *words)
        .unwrap_or(&[])
}

/// Below this many characters, a token is dropped outright — a bare
/// single-letter survivor ("a", "i", "e", "o", "y" — real one-letter words in
/// several of these languages) is too little to ever be evidence on its own
/// and is noise in every language at once. Deliberately NOT the boundary that
/// excludes `ci`/`vi`/`io`/`ha`/`da`/`ti` — see [`AMBIGUOUS_SHORT_TOKENS`] for
/// why those are a CURATED denylist, not a length rule: a blanket floor of 3
/// was tried first and measured to ALSO exclude Italian's own core vocabulary
/// (`il`, `la`, `di`, all 2 characters), which is exactly how short a real
/// Romance-language sentence's articles and prepositions are — a genuinely
/// Italian AWARDS section (`a_drifted_awards_section_warns_rather_than_blocks`)
/// dropped from 3+ distinctive hits to 1 under that rule and stopped firing.
const MIN_DISTINCTIVE_TOKEN_CHARS: usize = 2;

/// Tokens excluded from evidence in EVERY language, by name rather than by
/// length: each one is a genuine short function word in ONE curated language
/// (`ha`/`di`/`ci`/`io`/`vi`/`da`/`ti` → Italian; `os`/`em` → Portuguese;
/// `am`/`im`/`zu` → German; `el` → Spanish; `na` → Dutch; `ai`/`et`/`au` →
/// French) that is ALSO a common English abbreviation, initial, editor name
/// or URL/TLD fragment — `ci/cd`, the `vi` editor, `.io` domains, an "HA
/// cluster", the letters "AM"/"IM". Matching free text (unlike a whole-line
/// structural gate — `documents::evidence::entry`'s `DATE_ONLY_MARKERS`
/// excludes Dutch `nu` for the identical reason) has no other way to exclude
/// them, so they are named individually rather than caught by a length rule
/// that would also catch every OTHER 2-character word in these languages —
/// most of which (`il`, `la`, `de`, `un`, `le`) carry no comparable collision
/// risk and are exactly the evidence [`MIN_DISTINCTIVE_TOKEN_CHARS`]'s doc
/// explains a blanket floor cannot afford to lose. "per" (a genuine
/// 3-character English preposition ALSO used by Italian) is the one
/// short-token false positive measured with a length other than 2 — it is
/// closed instead by curating "per" into [`FUNCTION_WORDS_EN`], so it
/// collides out at the ordinary pairwise step rather than needing a name
/// here.
const AMBIGUOUS_SHORT_TOKENS: &[&str] = &[
    "ha", "di", "ci", "io", "vi", "da", "ti", "os", "em", "am", "im", "zu", "el", "na", "ai", "et",
    "au",
];

/// Below this many pairwise hits, a match is noise rather than evidence — a
/// single surviving token could be an incidental collision or a name.
const MIN_DISTINCTIVE_HITS: usize = 2;

/// How many times `text` uses a function word that belongs to `lang`'s
/// curated vocabulary but NOT to `other`'s — PAIRWISE against the specific
/// language being compared, not a single pool pruned across all seven at
/// once.
///
/// **Why pairwise, not a global pool (measured regression, not a hypothetical
/// one).** A prior version of this function pooled all seven languages'
/// vocabulary together and dropped any word appearing in two or more lists
/// before counting anything. Spanish and Portuguese are close enough to
/// French/Italian/Dutch that this pruned them down to 30 and 33 survivors
/// respectively — missing `de`, `la`, `un`, `en`, `a`, `con`, `por`, `para`,
/// `que`, `se`, the highest-frequency words in the language — and genuinely
/// Spanish/Portuguese text against a non-Spanish/Portuguese target regressed
/// to under-count or zero evidence, silently un-firing a real mismatch (see
/// `spanish_and_portuguese_regression` in the test module for the measured
/// numbers in both a flowing-prose and a bullets-plus-date-column register).
/// Pairwise pruning only removes a word from `lang`'s evidence when `other`
/// —the language ACTUALLY being compared against, i.e. normally the
/// document's real target — shares it; a word `lang` shares with some THIRD
/// curated language `other` has never heard of is not ambiguous for this
/// specific comparison and stays.
///
/// **A hit sandwiched between two Title-Case neighbours is excluded** — a
/// connector word ("der", "und", "für") glued between two capitalised words
/// is almost always sitting INSIDE a proper noun (an institution's own
/// official compound name — "Fachhochschule für Technik und Wirtschaft
/// Berlin" genuinely contains "für" and "und"), not free-standing prose
/// usage. Measured: an otherwise-English EDUCATION entry that spells out
/// such a name — the realistic shape, matching this crate's own fixture
/// convention of a terse "Degree, Institution, Dates" line — cleared the
/// evidence floor without this exclusion; genuinely wrong-language PROSE
/// (the Spanish/Portuguese regression fixtures, the existing wrong-language
/// résumé fixtures) is unaffected, because a real sentence surrounds its
/// function words with ordinary lowercase words, not two proper-noun
/// neighbours in a row. Costs the SAME direction every other guard in this
/// module costs: a genuinely wrong-language document whose every function
/// word happens to fall between two capitalised neighbours would lose that
/// evidence too — accepted, because the alternative (counting it) is the
/// false accusation this whole mechanism exists to prevent, and a document
/// that short and that proper-noun-dense was already a poor candidate for a
/// confident read (see [`MIN_CHARS_FOR_LANGUAGE_CHECK`]).
pub(super) fn pairwise_evidence_count(text: &str, lang: &str, other: &str) -> usize {
    let lang_words: HashSet<&str> = function_words_for(lang).iter().copied().collect();
    if lang_words.is_empty() {
        return 0;
    }
    let other_words: HashSet<&str> = function_words_for(other).iter().copied().collect();
    let tokens: Vec<&str> = text
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let is_title_case = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    tokens
        .iter()
        .enumerate()
        .filter(|(_, w)| w.chars().count() >= MIN_DISTINCTIVE_TOKEN_CHARS)
        .filter(|(_, w)| {
            let lower = w.to_lowercase();
            lang_words.contains(lower.as_str())
                && !other_words.contains(lower.as_str())
                && !AMBIGUOUS_SHORT_TOKENS.contains(&lower.as_str())
        })
        .filter(|(i, _)| {
            let prev_titlecase = *i > 0 && is_title_case(tokens[i - 1]);
            let next_titlecase = i + 1 < tokens.len() && is_title_case(tokens[i + 1]);
            !(prev_titlecase && next_titlecase)
        })
        .count()
}

/// Whether `lang` is one of the nine languages whose confident whatlang read
/// needs [`distinctive_evidence_confirms`]'s corroboration before it counts
/// as a mismatch — see [`LATIN_SCRIPT_LANGUAGES`]'s doc for why this is a
/// SCRIPT gate, not a curation gate.
pub(super) fn needs_distinctive_evidence(lang: &str) -> bool {
    LATIN_SCRIPT_LANGUAGES.contains(&lang)
}

/// Whether `found` — the language `detected_language(text)` actually named —
/// is corroborated well enough by `text` itself to accuse it of NOT being
/// `target`.
///
/// Two bars, both required:
///
/// 1. **Absolute floor** — [`pairwise_evidence_count`] for `found` against
///    `target` must clear [`MIN_DISTINCTIVE_HITS`]. Positive evidence only:
///    this never asks whether `target`'s own words are ABSENT, which would
///    reopen the original false Critical (a sparse-function-word document
///    genuinely written in `target`) by a different route.
/// 2. **Comparative margin** — `found`'s evidence must exceed `target`'s OWN
///    pairwise evidence in the SAME text, not just clear the floor in
///    isolation. Mechanically verified directly
///    (`distinctive_evidence_confirms_requires_a_real_margin_not_a_tie` in the
///    test module: a real 2-vs-2 tie does not confirm, a 3-vs-2 margin does).
///    The reasoning it exists for: a single, bigger absolute floor cannot
///    separate "an otherwise-English document that happens to name a foreign
///    institution" (a few incidental hits) from "genuinely sparse
///    Spanish/Portuguese text" (see [`pairwise_evidence_count`]'s doc) —
///    raising the floor to survive the first deepens the second. A
///    comparison against `target`'s OWN evidence in the SAME text separates
///    them instead: in the proper-noun case `target`'s evidence is abundant;
///    in the genuine-foreign-language case it is near zero, so even thin
///    `found` evidence clears it. **Measured limit, reported rather than
///    concealed:** the title-case-sandwich exclusion in
///    [`pairwise_evidence_count`] independently silences every REALISTIC
///    proper-noun fixture this module's test suite constructs (a terse
///    "Degree, Institution, Dates" line, or a longer multi-sentence label), so
///    this bar currently has no end-to-end regression that isolates it from
///    that exclusion — a deliberately German-/French-institution-dense search
///    for one either stayed confidently English overall (never reaching this
///    branch) or produced enough real foreign-language evidence to be a
///    defensible true positive, not a tie. Kept as a principled second layer
///    (a bare floor is measurably weaker in the abstract) with direct,
///    mechanical coverage rather than removed for lack of a natural trigger.
///
/// A non-Latin-script `found` (gated by [`needs_distinctive_evidence`]) skips
/// both bars — whatlang's script read is already reliable there regardless of
/// function-word density, and requiring evidence this crate has no
/// vocabulary for would only turn a genuine mismatch quiet.
pub(super) fn distinctive_evidence_confirms(text: &str, found: &str, target: &str) -> bool {
    if !needs_distinctive_evidence(found) {
        return true;
    }
    let found_evidence = pairwise_evidence_count(text, found, target);
    if found_evidence < MIN_DISTINCTIVE_HITS {
        return false;
    }
    let target_evidence = pairwise_evidence_count(text, target, found);
    found_evidence > target_evidence
}

/// Whether `text` is confidently written in something other than `lang`.
///
/// Three independent reasons to go quiet, all already the module's stated
/// posture: too SHORT to read a language from at all
/// ([`MIN_CHARS_FOR_LANGUAGE_CHECK`]); [`detected_language`] itself is not
/// confident (below `documents::keywords::MIN_DETECTION_CONFIDENCE`) or reads
/// a language this crate does not curate; or `whatlang` confidently names a
/// Latin-script language ([`needs_distinctive_evidence`]) without
/// [`distinctive_evidence_confirms`] backing it up. That third gate is what
/// closes the confident-but-wrong noun-phrase misread the module doc above
/// measures: a whatlang guess in the Latin ambiguity zone is corroborated by
/// actual, comparative function-word evidence before it becomes an
/// accusation, exactly the way `target_is_corroborated` already corroborates
/// the TARGET side. Every other reason to go quiet is unchanged. `None`/
/// `false` never counts as a mismatch — an unreliable or uncorroborated read
/// cannot manufacture an accusation, it can only fail to make one.
pub(super) fn is_language_mismatch(text: &str, lang: &str) -> bool {
    if significant_chars(text) < MIN_CHARS_FOR_LANGUAGE_CHECK {
        return false;
    }
    let Some(found) = detected_language(text) else {
        return false;
    };
    found != lang && distinctive_evidence_confirms(text, found, lang)
}

/// Whether an independent witness — the job ad, or the candidate's own source
/// résumé — confidently reads as `lang` too, so `lang` itself is credible
/// enough to accuse a document of failing to match it.
///
/// Replaces the old `source_is_a_reliable_control`, which required the SOURCE
/// specifically to already read as the target — true only when no
/// translation was needed, i.e. false in exactly the cross-language case this
/// whole check exists to catch (an English source résumé, a German target).
/// English source + `target_language: "de"` used to make BOTH witnesses fail
/// by construction; corroboration asks a document-agnostic question instead
/// — "is `lang` real" — so a translation run is no longer disqualified from
/// having its own translation graded.
///
/// No length floor on either witness: `detected_language`'s own confidence
/// gate is a better-calibrated reliability signal than a raw character count
/// (see its doc comment) and already absorbs the false-positive risk the old
/// control's floor existed for — a terse ad or a short certifications block
/// reads at confidence 0.08–0.13 in this crate's own fixtures, comfortably
/// below the 0.9 bar, so `detected_language` already answers `None` for them
/// without a second, redundant length check here.
fn target_is_corroborated(job_ad: &str, source_resume: &str, lang: &str) -> bool {
    detected_language(job_ad) == Some(lang) || detected_language(source_resume) == Some(lang)
}

/// Whether `generated` came back in the wrong language for `target_language`,
/// given `source_resume` and `job_ad` as witnesses that the target itself is
/// real (see [`target_is_corroborated`]).
///
/// The single answer to "did this run come back in the wrong language" —
/// [`super::language_issues`] uses it (via [`Analysis::language_mismatch`])
/// for the deterministic Critical, and the pipeline's draft-retry
/// (`pipeline::resume::stages::draft`) is meant to call this SAME function
/// before spending a second model call, so `validate` and the retry guarding
/// against the same defect can never quietly disagree about what "wrong
/// language" means.
pub fn document_language_mismatch(
    generated: &str,
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
) -> bool {
    let lang = super::normalize_language(target_language);
    is_language_mismatch(generated, &lang) && target_is_corroborated(job_ad, source_resume, &lang)
}

/// `content.language_mismatch` — the output is not in the language it was asked
/// for. Critical: a German résumé sent to an English-speaking employer is not a
/// quality nit, and every downstream comparison is meaningless once it holds.
///
/// Two passes, in order. The DOCUMENT pass is [`Analysis::language_mismatch`]
/// (a whole-text read via [`document_language_mismatch`]) — reliable at that
/// scale, but it takes roughly a third of a nine-section résumé drifting to a
/// minority language before the vote flips; one drifted section, or two, reads
/// as noise inside a long document and never fires. So when the document reads
/// clean, a SECTION pass checks each section on its own — the shape the
/// reported defect actually takes ("summary in English, experience in Italian,
/// skills in English again"). Both passes route through the same
/// `detected_language` kernel — this is a second SCOPE the language question
/// is asked over, not a second answer to it.
pub(super) fn language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if ctx.language_mismatch {
        return vec![issue(
            CONTENT_LANGUAGE_MISMATCH,
            None,
            format!(
                "This document does not read as {}, the language it was generated for. \
                 Re-generate it with the target language set correctly before sending.",
                ctx.lang
            ),
            None,
        )];
    }
    section_language_issues(ctx)
}

/// Share of `text`'s alphabetic-initial words that open LOWERCASE — the
/// discriminator [`section_language_issues`] uses in place of a heading-kind
/// allowlist to decide whether a section reads as PROSE at all.
///
/// A list item — a certification, a tool name, a keyword line — is written in
/// Title Case almost without exception. A sentence in any language this
/// pipeline supports is not, however heavily it capitalizes its OWN nouns
/// (German capitalizes every one): it still connects them with lowercase
/// articles, prepositions, pronouns and verbs. So the signal is not case
/// itself (language-dependent) but how MUCH of the text sits in lowercase —
/// exactly the function-word density `whatlang` needs to read a language from
/// at all, without needing to know what that language is first.
///
/// *Accepted cost, in both directions:*
///
/// - A lowercase technical list under a heading `classify_section` does NOT
///   recognize as [`SectionKind::Skills`] ("kafka, postgresql" under
///   Experience) reads as prose here — canonical tool-name casing (`pandas`,
///   `git`, `nginx`) IS lowercase, so this is measured, not hypothetical.
///   The call site excludes `SectionKind::Skills` outright (belt and braces)
///   for exactly this reason — and it is NOT redundant with
///   `detected_language`'s confidence gate: a comma/middot tool list can read
///   as a covered-but-wrong language with confidence 1.0 (measured: whatlang
///   reads a lowercase Python/data-tooling list as confident French), so the
///   confidence gate alone does not suppress it.
/// - A caseless script (Arabic, Hebrew, CJK, Thai, Devanagari, …) has no
///   uppercase/lowercase distinction at all, so a whole-TEXT ratio would
///   always read 0 and silently skip it — worse, since a real drift then
///   goes unreported. A caseless section still carries a Latin heading word
///   ahead of it in [`section_text`]'s output ("EXPERIENCE" over an Arabic
///   body), which would poison a whole-text caseless check too, so
///   [`looks_like_prose`] decides PER WORD: a word with no case distinction
///   counts toward the lowercase side, same as a genuine lowercase word.
pub(super) const PROSE_LOWERCASE_WORD_RATIO: f64 = 0.2;

pub(super) fn looks_like_prose(text: &str) -> bool {
    let mut words = 0usize;
    let mut lowercase_initial = 0usize;
    for word in text.split_whitespace() {
        let Some(first) = word.chars().next() else {
            continue;
        };
        if !first.is_alphabetic() {
            continue; // A bare number, a bullet marker, a parenthesised year.
        }
        words += 1;
        // A caseless-script word can never "open Title Case" the way a Latin
        // word can, so it counts as lowercase too — see the doc above.
        let has_case = word.chars().any(|c| c.is_uppercase() || c.is_lowercase());
        if first.is_lowercase() || !has_case {
            lowercase_initial += 1;
        }
    }
    words > 0 && (lowercase_initial as f64 / words as f64) >= PROSE_LOWERCASE_WORD_RATIO
}

/// Per-section half of [`language_issues`].
///
/// Gated on **two** independent conditions, both required — dropping either
/// one reopens a real false-Critical shape, so neither is a substitute for
/// the other:
///
/// 1. `target_is_corroborated(ctx.input.job_ad, ctx.input.source_resume,
///    &ctx.lang)` — the target language itself must be credible before a
///    section can be accused of failing to match it. Without this, a
///    document written in ANY language — including a genuine, correct
///    translation into a THIRD language neither witness names — can still
///    get a section flagged against a `target_language` nothing actually
///    corroborates (measured: an unreliable/short job ad plus a
///    differently-language source resume corroborates nothing, yet a
///    drifted section still read as "wrong" against an uncorroborated
///    target before this guard existed). This is the document-level check's
///    own [`target_is_corroborated`] call, applied here too — the document
///    pass and the section pass must agree on whether `lang` is real, not
///    just on whether the text matches it.
/// 2. `!is_language_mismatch(ctx.input.generated, &ctx.lang)` — the document
///    as a whole must not CONFIDENTLY read as the wrong language. That is
///    deliberately weaker than requiring it to confidently read as RIGHT: a
///    document with exactly one drifted section is, BY CONSTRUCTION, the
///    shape whose whole-text confidence a section-level check most needs to
///    survive. Measured on this crate's own fixture (one Italian EXPERIENCE
///    section inside an otherwise-English résumé): the whole document reads
///    at confidence 0.28, well under `MIN_DETECTION_CONFIDENCE` — so gating
///    on "the whole document confidently reads as `lang`" would have made
///    THIS pass switch itself off in exactly the one case the reported
///    defect actually takes ("summary in English, experience in Italian,
///    skills in English again"): the more a document drifts, the LESS
///    confident the whole-text read becomes, which would make the two
///    mechanisms cancel each other out. `is_language_mismatch` on its own
///    goes quiet on that same low-confidence read (`None` never counts as a
///    mismatch), so condition 2 opens instead of closing — the section pass
///    gets to look for exactly what corrupted the document-level confidence
///    in the first place. Condition 2 ALONE, however, is not sufficient: it
///    only asks "is the document confidently wrong", never "is the target
///    even real" — a document that is neither confidently wrong NOR backed
///    by any corroborating witness would sail through condition 2 and still
///    get a section flagged, which is exactly the gap condition 1 closes.
///
/// Replaces the old `source_is_a_reliable_control` gate for the same reason
/// [`target_is_corroborated`] does — that control read the SOURCE résumé
/// specifically, which fails open precisely when a translation was expected
/// (an English source, a German target: the source was never going to read
/// as German). Condition 2 reads the GENERATED document instead, so it has
/// no translation blind spot on its own; condition 1 (the corroboration
/// requirement) is what the old control was actually FOR, restored here in
/// its document-agnostic form. A document that confidently reads as some
/// THIRD, uncorroborated language — or an uncorroborated target full stop —
/// skips both the document- and section-level passes together, the same
/// "goes quiet on a real disagreement" posture this whole module takes.
///
/// Two more guards a section-scoped read needs that the document-scoped one
/// does not, both TRADED conservatively toward missing a defect rather than
/// accusing a truthful section:
///
/// * the SAME [`MIN_CHARS_FOR_LANGUAGE_CHECK`] floor, applied per section;
/// * a [`SectionKind::Skills`] exclusion, AND only sections that
///   [`looks_like_prose`] on top of that — belt and braces, not either
///   alone. The allowlist this replaced was a heading-KIND check
///   (Summary/Experience/Projects) too NARROW to see a drifted "Work
///   History"/"Selected Roles" section at all; `looks_like_prose` widens
///   coverage to any sentence-shaped section, including ones
///   `classify_section` can't name, but it is not a substitute for the
///   Skills exclusion (see the false-positive note on [`PROSE_LOWERCASE_WORD_RATIO`]).
///
///   The cost, paid deliberately: a model that drifts ONLY a list-shaped
///   section is not caught here (the document-level pass still can, if enough
///   of the rest drifts too) — the same trade the kind allowlist already made.
fn section_language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !target_is_corroborated(ctx.input.job_ad, ctx.input.source_resume, &ctx.lang)
        || is_language_mismatch(ctx.input.generated, &ctx.lang)
    {
        return Vec::new();
    }
    ctx.generated_sections
        .iter()
        .skip(1) // section 0 is the header band (name + contact), not prose
        .filter(|section| section.kind != SectionKind::Skills)
        .filter_map(|section| {
            let heading = section.heading.as_deref()?;
            let body = section_text(section);
            if significant_chars(&body) < MIN_CHARS_FOR_LANGUAGE_CHECK {
                return None;
            }
            if !looks_like_prose(&body) {
                return None;
            }
            // Routed through the SAME `is_language_mismatch` the document pass
            // uses (rather than a second, duplicated `detected_language`
            // comparison) so the distinctive-function-word corroboration
            // above applies here too — a section-scoped noun-phrase block
            // (a terse "Certifications" heading `classify_section` files as
            // `Other`, say) is exactly as vulnerable to whatlang's
            // confident-but-wrong misread as the whole document is.
            if !is_language_mismatch(&body, &ctx.lang) {
                return None;
            }
            let mut found = issue(
                CONTENT_LANGUAGE_MISMATCH,
                Some(heading),
                format!(
                    "The \"{heading}\" section does not read as {}, the language the rest of \
                     this document is written in. Re-generate it (or that section) before \
                     sending.",
                    ctx.lang
                ),
                Some(ctx.lang.clone()),
            );
            // `Other` (Volunteer/Awards/…) has no `SectionKey` — downgraded so it surfaces, not blocks.
            if section.kind == SectionKind::Other {
                found.severity = Severity::Warning;
            }
            Some(found)
        })
        .collect()
}

/// A section's own text — heading plus every line beneath it, one per line —
/// for a check that reads the section as one span rather than line by line.
fn section_text(section: &Section) -> String {
    let mut text = String::new();
    if let Some(heading) = &section.heading {
        text.push_str(heading);
        text.push('\n');
    }
    for line in &section.lines {
        text.push_str(&line.text);
        text.push('\n');
    }
    text
}
