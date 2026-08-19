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
//! that the whatlang-named language is present in the text at all — before a
//! confident Latin-script read becomes an accusation. See
//! [`pairwise_evidence_count`] for why the check is pairwise (found vs.
//! target only, not a single global pool pruned across all seven languages at
//! once): a global pool was measured to leave Spanish/Portuguese too thin to
//! evidence themselves — a true-positive regression, not a hypothetical one.
//!
//! Two OTHER suppressors were tried on top of the pairwise floor — a curated
//! denylist for short tokens that double as English abbreviations
//! (`ci`/`vi`/`io`/`ha`…), and a comparative margin requiring `found`'s
//! evidence to EXCEED `target`'s own — and both were REMOVED after
//! measurement found no realistic document that needed either: every
//! behavioural test through `validate_content` passed with each one disabled,
//! only its own manufactured unit test failed, and the denylist's one
//! measured trigger (French idiom words stuffed together with no connecting
//! English grammar) was adversarial, not a shape this validator's real input
//! — generated or user-uploaded résumé/letter prose — ever produces. Removed
//! rather than kept on the strength of the argument for them, per this
//! module's own history: the original global pool was equally defensible on
//! paper and silently killed the Spanish Critical anyway.
//!
//! A THIRD suppressor — a title-case-sandwich exclusion in
//! [`pairwise_evidence_count`], dropping a hit whose immediate neighbours
//! were both capitalised — was tried, shipped, and then REMOVED after a
//! pre-PR gate found it deleted German's evidence wholesale: standard German
//! CV register is nominal ("Aufbau und Betrieb der Zahlungsplattform"), and
//! German capitalises every noun, so in that register every connector sits
//! between two capitalised nouns whether or not it is inside a proper noun.
//! [`MIN_DISTINCTIVE_HITS`] is what replaced it — a threshold, not a shape
//! rule, chosen from a corpus swept fresh for this removal; see its own doc
//! for the full table and the residual risk it accepts instead of hiding.

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
///
/// **Not the same primitive as `documents::evidence`'s `FUNCTION_WORDS_DE` /
/// `has_curated_function_words`, despite the identical const name (for
/// German) and overlapping content — do not "unify" them.** That module asks
/// a SKILL-CLAIM question (which tokens on a skills line are filler, not a
/// claimed skill) and is deliberately curated for German only, since an
/// under-curated list there is safe (a noisier display-only gap list) but an
/// over-eager one is not. This module asks a language-IDENTITY question
/// (does `text` carry positive evidence of some OTHER specific curated
/// language) across all seven languages `documents::keywords::make_stemmer`
/// stems for — a missing word here only makes the guard quieter, never
/// louder, so exhaustiveness matters less than not accusing wrongly. Same
/// shape of list, different correctness bar; merging them would let a change
/// tuned for one silently regress the other.
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
///
/// A DIFFERENT question from `documents::evidence::has_curated_function_words` —
/// that one answers `false` for `"es"`/`"fr"`/`"it"`/`"nl"`/`"pt"` (no skill-
/// filler list written for them) while this function returns 61/68/65/58/58
/// real words for the same five tags; see [`FUNCTION_WORDS_EN`]'s doc for why
/// the two primitives are deliberately separate rather than one the other
/// could read from.
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
/// and is noise in every language at once. Deliberately not higher: a blanket
/// floor of 3 was tried and measured to ALSO exclude Italian's own core
/// vocabulary (`il`, `la`, `di`, all 2 characters), which is exactly how short
/// a real Romance-language sentence's articles and prepositions are — a
/// genuinely Italian AWARDS section
/// (`a_drifted_awards_section_warns_rather_than_blocks`) dropped from 3+
/// distinctive hits to 1 under that rule and stopped firing.
///
/// **A curated denylist for the residual short-token ambiguity
/// (`ci`/`vi`/`io`/`ha`/`da`/`ti`/… — genuine short function words in one
/// curated language that are ALSO common English abbreviations, editor names
/// or TLD fragments: `ci/cd`, the `vi` editor, `.io`, an "HA cluster") was
/// tried on top of this floor and REMOVED after measurement.** The
/// found-SPECIFIC pairwise match already closes every one of those cases on
/// its own — none of `ci`/`vi`/`io`/`ha`/`da`/`ti` is a word in French, and
/// whatlang's guess on the tool-list fixture those tokens were measured
/// against never changed to Italian — so the denylist's only demonstrated
/// trigger was an adversarial string of disconnected French idioms
/// ("au naturel au revoir au contraire …", no connecting English grammar at
/// all) that does not resemble anything this validator's real input
/// (generated or user-uploaded résumé/letter prose) produces; the same
/// phrases used as genuine English loanwords in an ordinary sentence never
/// reached the vulnerable state. Kept out rather than kept anyway: it would
/// have permanently denied `el`/`zu`/`na`/`et`/`di`/`ci` — high-frequency
/// function words of Spanish, German, Dutch, French and Italian — sitting one
/// hit from the margin the Spanish/Portuguese fix above needed.
const MIN_DISTINCTIVE_TOKEN_CHARS: usize = 2;

/// Below this many pairwise hits, a match is noise rather than evidence — a
/// single surviving token could be an incidental collision or a name.
///
/// **This is the ONLY discriminator left; a shape-based heuristic (a
/// title-case-sandwich exclusion) was tried here and REMOVED after it
/// deleted German's evidence wholesale.** Standard German CV register is
/// nominal ("Aufbau und Betrieb der Zahlungsplattform", not "Ich habe die
/// Zahlungsplattform aufgebaut und betrieben"): German capitalises every
/// noun, so in this register EVERY connector sits between two capitalised
/// nouns by construction, not because it is inside a proper noun — the
/// exclusion could not tell those apart and fired on nothing for a whole
/// nominal-register German résumé. This module's own history is now three
/// suppressors in a row, each a shape assumption that held for some
/// languages and broke for one it had not been checked against (a globally
/// pruned pool assumed collisions "remove themselves" — wrong for Spanish; a
/// short-token denylist assumed length implies ambiguity — unfalsifiable; a
/// title-case exclusion assumed a function word between capitals is inside a
/// proper noun — wrong for German). A threshold makes no shape assumption,
/// only a quantity one, so it is what remains.
///
/// **5, chosen from a corpus swept fresh for this change (not carried over
/// from an earlier round's number), reported in full because the failure
/// mode this crate has now hit three times is calibrating a threshold
/// against the fixture that tests it:**
///
/// | class | fixture | evidence |
/// |---|---|---|
/// | quiet | English noun-phrase tool list (the original bug) | 0 |
/// | quiet | same list + ci/cd, .io, vi, HA, or per (the short-token leaks) | 0 |
/// | quiet | German institution name, terse "Degree, Institution, Dates" | 1 |
/// | quiet | German institution name, EDUCATION section (2 entries) | 3 |
/// | quiet | German institution names, CERTIFICATIONS (3 entries) | 4 |
/// | fire | short genuine Spanish paragraph (2 sentences) | 6 |
/// | fire | Italian VOLUNTEER section (PR #1003's own fixture) | 6 |
/// | fire | Italian AWARDS section (PR #1003's own fixture) | 8 |
/// | fire | short genuine nominal-register German bullet (1 line) | 9 |
/// | fire | Portuguese EXPERIENCE, prose or bullets+date-column | 14 |
/// | fire | French, Title-Case rendered | 15 |
/// | fire | Spanish EXPERIENCE, prose or bullets+date-column | 16–18 |
/// | fire | Dutch, Title-Case rendered | 16 |
/// | fire | genuine third-language Italian output | 17 |
/// | fire | Italian EXPERIENCE section (drifted-section fixture) | 19 |
/// | fire | nominal-register German, whole résumé or one section | 20–24 |
/// | fire | German verb-register résumé (the original reported bug) | 24 |
///
/// Quiet tops out at 4, fire starts at 6 — a floor of 5 separates this
/// corpus with a full unit of margin on both sides, matching (independently,
/// not by construction) the separation an earlier review measured on a
/// different fixture set.
///
/// **Residual risk, measured and accepted rather than hidden:** an
/// adversarially INSTITUTION-DENSE English CERTIFICATIONS section — five
/// DIFFERENT German institutions in one list, a shape rarer than the
/// 3-institution case above — reaches evidence 8, ABOVE both the floor and
/// the thinnest genuine positive (6). A pure count cannot separate "an
/// unusually credentialed English document" from "a short genuine foreign
/// paragraph" in every case, because evidence scales with how much text
/// there is, not with whether that text is target-language prose or
/// accumulated foreign proper nouns. Raising the floor to close this would
/// silence the genuinely thin true positives it sits right next to (6);
/// this module already treats a false negative as the worse failure three
/// times over, so the floor stays at 5 and this gap is accepted rather than
/// closed by a shape rule that would just be the next one this crate
/// discovers is wrong for some language.
const MIN_DISTINCTIVE_HITS: usize = 5;

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
/// **A title-case-sandwich exclusion (dropping a hit whose immediate
/// neighbours were BOTH capitalised, on the theory that it sits inside a
/// proper noun) was tried here and REMOVED — it was a false generalisation
/// from Latin languages to German.** Standard German CV register is nominal
/// ("Aufbau und Betrieb der Zahlungsplattform", "Leitung der Migration der
/// Dienste"): German capitalises every noun, so in this register EVERY
/// connector sits between two capitalised nouns by construction, not
/// because it is part of a proper noun. The exclusion deleted German's
/// evidence wholesale — measured: a whole nominal-register German résumé
/// against an English target dropped to 1 pairwise hit and stopped firing,
/// with no second line of defence, because both the document and section
/// passes route through this same function. See [`MIN_DISTINCTIVE_HITS`]'s
/// doc for the threshold-only replacement and the corpus it was chosen
/// from.
pub(super) fn pairwise_evidence_count(text: &str, lang: &str, other: &str) -> usize {
    let lang_words: HashSet<&str> = function_words_for(lang).iter().copied().collect();
    if lang_words.is_empty() {
        return 0;
    }
    let other_words: HashSet<&str> = function_words_for(other).iter().copied().collect();
    text.split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= MIN_DISTINCTIVE_TOKEN_CHARS)
        .filter(|w| {
            let lower = w.to_lowercase();
            lang_words.contains(lower.as_str()) && !other_words.contains(lower.as_str())
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
/// `target`: [`pairwise_evidence_count`] for `found` against `target` must
/// clear [`MIN_DISTINCTIVE_HITS`]. Positive evidence only: this never asks
/// whether `target`'s own words are ABSENT, which would reopen the original
/// false Critical (a sparse-function-word document genuinely written in
/// `target`) by a different route.
///
/// **A second bar — requiring `found`'s evidence to EXCEED `target`'s own
/// pairwise evidence in the same text, not just clear the floor — was tried
/// and REMOVED after measurement.** It was built to separate "an otherwise-
/// English document that merely names a foreign institution" from "genuinely
/// sparse Spanish/Portuguese text", but at the time that separation turned
/// out to already be a title-case-sandwich exclusion's job: disabling the
/// comparative bar alone left every `validate::content` behavioural test
/// green, and left only its own manufactured unit test red. Removed rather
/// than kept on the strength of the argument for it — the argument was real,
/// the measurement said it carried no load. (The title-case exclusion it
/// deferred to was ITSELF removed one round later, for breaking German —
/// see [`MIN_DISTINCTIVE_HITS`]'s doc; nothing has replaced either bar
/// except the single floor below.)
///
/// A non-Latin-script `found` (gated by [`needs_distinctive_evidence`]) skips
/// this floor entirely — whatlang's script read is already reliable there
/// regardless of function-word density, and requiring evidence this crate has
/// no vocabulary for would only turn a genuine mismatch quiet.
pub(super) fn distinctive_evidence_confirms(text: &str, found: &str, target: &str) -> bool {
    if !needs_distinctive_evidence(found) {
        return true;
    }
    pairwise_evidence_count(text, found, target) >= MIN_DISTINCTIVE_HITS
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
/// actual function-word evidence before it becomes an accusation, exactly the
/// way `target_is_corroborated` already corroborates the TARGET side. Every
/// other reason to go quiet is unchanged. `None`/
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
