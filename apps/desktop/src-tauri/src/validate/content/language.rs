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
//! [`distinctive_language_evidence`] — real, collision-pruned function-word
//! evidence of some OTHER curated language — before a confident Latin-script
//! read becomes an accusation. See that function's doc for why it is a
//! POSITIVE check, never an absence test, and why it is scoped to Latin
//! scripts only.

use std::collections::HashMap;

use super::{
    issue, significant_chars, Analysis, ContentIssue, Section, SectionKind, Severity,
    CONTENT_LANGUAGE_MISMATCH,
};
use crate::documents::keywords::detected_language;

/// Below this many non-whitespace characters, `whatlang` guesses. A Critical
/// language mismatch on a two-line draft would be a false accusation, so the
/// check goes quiet instead.
pub(super) const MIN_CHARS_FOR_LANGUAGE_CHECK: usize = 120;

/// Function words distinctive to ONE Latin-script curated language — the
/// discriminator [`is_language_mismatch`] needs on top of whatlang's raw
/// guess.
///
/// `whatlang`'s `confidence()` is a top-1-vs-top-2 MARGIN over n-gram
/// statistics, not a correctness probability. A block of ordinary noun
/// phrases ("Administration / Supervision / Coordination / Optimisation /
/// Certification…") starves the n-gram model of the function words it needs
/// to read a language from at all, and it lands on some OTHER language with
/// MAXIMUM margin — `is_reliable() == true` and all (measured: a terse
/// English noun-phrase block reads as confident French). No signal derived
/// from how the OUTPUT relates to the SOURCE can rule this out — a genuine
/// translation and a false misread of a truthful document read identically on
/// that axis, because the two differ only in what the TARGET actually is. The
/// fix has to be a better language-identity read for exactly the register
/// whatlang is weak on: real prose is dense with closed-class function words
/// (articles, prepositions, pronouns, copulas); a noun-phrase list is not.
///
/// **Deliberately SEPARATE from `documents::evidence::function_words` /
/// `has_curated_function_words`.** That pair feeds the ATS keyword-density
/// and skill-not-demonstrated checks, where the failure mode is a MISSING
/// filler word (an unremoved German "Kenntnisse" reads as a fake keyword) —
/// so every curated language there needs an exhaustive list, and an
/// under-curated one is unsafe. Here the failure mode runs the OTHER way: an
/// under-curated list just produces LESS evidence and this check goes quiet
/// (safe); an OVER-eager one — a word claimed distinctive when another
/// language uses it too — manufactures a false accusation (unsafe). So the
/// bar here is "correct after pruning collisions", not "exhaustive", and the
/// two lists must never be conflated or reused for each other's purpose —
/// reusing this list for the ATS checks would under-cover them (they need
/// every filler, not just the collision-pruned remainder), and reusing theirs
/// here would double-count words this module never authored against a
/// pruning invariant.
///
/// Collisions are real and common across these seven languages: "de" is a
/// Dutch article and a French/Spanish/Portuguese preposition; "a"/"in" are
/// English and Italian/Spanish; "la" is French, Spanish and Italian. Rather
/// than hand-picking around each one, every list below is written honestly
/// (the words a fluent speaker would actually list), and
/// [`distinctive_pool`] pools all seven and drops anything that lands in more
/// than one — so a genuine collision safely removes itself from evidence
/// instead of being guessed at.
const FUNCTION_WORDS_EN: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "so", "as", "of", "in", "on", "at", "to", "by",
    "for", "with", "from", "into", "onto", "about", "over", "under", "between", "through",
    "during", "before", "after", "this", "that", "these", "those", "it", "he", "she", "him", "her",
    "his", "they", "them", "their", "we", "us", "our", "you", "your", "i", "my", "is", "are",
    "was", "were", "be", "been", "being", "have", "has", "had", "do", "does", "did", "will",
    "would", "can", "could", "should", "must", "not", "no",
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
    "ons", "mijn", "jouw", "zijn", "hun", "is", "zijn", "was", "waren", "ben", "bent", "heb",
    "hebt", "heeft", "hebben", "had", "hadden", "wordt", "worden", "kan", "moet",
];
const FUNCTION_WORDS_PT: &[&str] = &[
    "o", "a", "os", "as", "um", "uma", "uns", "umas", "e", "ou", "mas", "porque", "que", "se",
    "embora", "nem", "de", "em", "a", "com", "por", "para", "sem", "sobre", "entre", "até",
    "desde", "eu", "tu", "ele", "ela", "nós", "vós", "eles", "elas", "me", "te", "se", "nos",
    "meu", "teu", "seu", "nosso", "vosso", "é", "são", "era", "eram", "sou", "és", "somos",
    "estar", "está", "estão", "ser", "foi", "foram", "tem", "têm", "há",
];

/// The seven languages [`is_language_mismatch`]'s corroboration check
/// curates — English plus the SAME six Snowball languages
/// `documents::keywords::make_stemmer` stems for, not the full nineteen
/// [`crate::documents::keywords::locale_tag_of`] recognises. This is the
/// whole Latin-script ambiguity zone the module doc above describes; every
/// other curated language reads a distinct SCRIPT, which whatlang already
/// gets right at near-1.0 confidence regardless of function-word density
/// (see [`is_latin_curated`]).
const CURATED_FUNCTION_WORDS: &[(&str, &[&str])] = &[
    ("en", FUNCTION_WORDS_EN),
    ("de", FUNCTION_WORDS_DE),
    ("fr", FUNCTION_WORDS_FR),
    ("es", FUNCTION_WORDS_ES),
    ("it", FUNCTION_WORDS_IT),
    ("nl", FUNCTION_WORDS_NL),
    ("pt", FUNCTION_WORDS_PT),
];

/// Below this many hits, a match is noise rather than evidence — a single
/// surviving token could be an incidental loanword or a name. Two rather than
/// one, and no higher: the tightest genuine case measured (see
/// `every_curated_language_is_silent_for_itself_and_fires_for_every_other`'s
/// Spanish fixture — after pruning, only "tiene" survives, three times over)
/// clears two with a full unit of margin; three would leave that fixture
/// exactly on the boundary.
const MIN_DISTINCTIVE_HITS: usize = 2;

/// Every token that appears in exactly one of [`CURATED_FUNCTION_WORDS`]'s
/// seven lists, mapped to that one language — collisions (appearing in two or
/// more) are dropped entirely rather than guessed at. Rebuilt on every call:
/// ~300 short strings, and this runs at most a handful of times per
/// `validate()` call (once for the document, once per prose section), not
/// worth caching.
fn distinctive_pool() -> HashMap<&'static str, &'static str> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for (_, words) in CURATED_FUNCTION_WORDS {
        for w in *words {
            *seen.entry(w).or_default() += 1;
        }
    }
    let mut pool = HashMap::new();
    for (lang, words) in CURATED_FUNCTION_WORDS {
        for w in *words {
            if seen[w] == 1 {
                pool.insert(*w, *lang);
            }
        }
    }
    pool
}

/// Whether `lang` is one of the seven languages whose confident-but-wrong
/// reads need [`distinctive_language_evidence`]'s corroboration before they
/// count as a mismatch — see [`CURATED_FUNCTION_WORDS`]'s doc. A non-Latin
/// SCRIPT never needs it: `whatlang` reads script alone at near-1.0
/// confidence regardless of function-word density (see
/// `documents::keywords::test`'s twelve-language non-Latin sweep), so
/// requiring evidence there would only make a genuine mismatch — a résumé
/// section that drifted to Arabic, say — go quiet for no reason.
fn is_latin_curated(lang: &str) -> bool {
    CURATED_FUNCTION_WORDS.iter().any(|(l, _)| *l == lang)
}

/// Whether `text` carries [`MIN_DISTINCTIVE_HITS`] or more occurrences of
/// function words distinctive to some ONE curated language other than
/// `target` — POSITIVE evidence that the text really is written in a
/// different language.
///
/// Never the other direction — this does not ask whether `target`'s own
/// words are ABSENT. That would reopen the exact false Critical this exists
/// to close by a different route: a terse but genuine target-language
/// document (few function words, by construction) would fail an absence
/// test the same way it confuses whatlang. Positive evidence only degrades
/// safely: an under-curated or genuinely short text just produces less of
/// it, and the guard stays quiet rather than guessing.
///
/// Answers only "is SOME other language evidenced", not "which one whatlang
/// guessed" — `is_language_mismatch` does not require this to name the same
/// language `detected_language` did; either language is real, unwanted
/// evidence.
fn distinctive_language_evidence(text: &str, target: &str) -> Option<&'static str> {
    let pool = distinctive_pool();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for word in text.split(|c: char| !c.is_alphabetic()) {
        if word.is_empty() {
            continue;
        }
        let lower = word.to_lowercase();
        if let Some(&lang) = pool.get(lower.as_str()) {
            if lang != target {
                *counts.entry(lang).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .find(|(_, n)| *n >= MIN_DISTINCTIVE_HITS)
        .map(|(lang, _)| lang)
}

/// Whether `text` is confidently written in something other than `lang`.
///
/// Three independent reasons to go quiet, all already the module's stated
/// posture: too SHORT to read a language from at all
/// ([`MIN_CHARS_FOR_LANGUAGE_CHECK`]); [`detected_language`] itself is not
/// confident (below `documents::keywords::MIN_DETECTION_CONFIDENCE`) or reads
/// a language this crate does not curate; or — new — `whatlang` confidently
/// names one of the seven Latin-script languages ([`is_latin_curated`])
/// without [`distinctive_language_evidence`] backing it up. That third gate
/// is what closes the confident-but-wrong noun-phrase misread the module doc
/// above measures: a whatlang guess in the Latin ambiguity zone is corroborated
/// by actual function-word evidence before it becomes an accusation, exactly
/// the way `target_is_corroborated` already corroborates the TARGET side.
/// Every other reason to go quiet is unchanged. `None`/`false` never counts as
/// a mismatch — an unreliable or uncorroborated read cannot manufacture an
/// accusation, it can only fail to make one.
pub(super) fn is_language_mismatch(text: &str, lang: &str) -> bool {
    if significant_chars(text) < MIN_CHARS_FOR_LANGUAGE_CHECK {
        return false;
    }
    let Some(found) = detected_language(text) else {
        return false;
    };
    found != lang
        && (!is_latin_curated(found) || distinctive_language_evidence(text, lang).is_some())
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
