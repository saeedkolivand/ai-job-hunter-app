//! Anti-AI-tell checks — does the output read as machine-written?
//!
//! Every rule here mirrors a ban the generation prompt already issued
//! (`packages/prompts/.../natural-voice.ts`, transcribed into
//! [`super::lexicon`]), so this is the deterministic check that the prompt was
//! actually obeyed rather than a second, independent opinion about style.
//!
//! All Warnings. Prose taste is not a defect, and a résumé that legitimately
//! contains the word "robust" is not broken.

use super::lexicon;
use super::{
    contains_phrase, flattened_lower, issue, sentences, word_count, Analysis, ContentIssue,
    VOICE_AI_TELL_LEXICAL, VOICE_EM_DASH_OVERUSE, VOICE_GENERIC_LETTER, VOICE_LOW_BURSTINESS,
    VOICE_RULE_OF_THREE_DENSITY, VOICE_TEMPLATE_OPENER,
};

/// Below this many sentences, sentence-length variance is not a signal — three
/// similar sentences are a coincidence, not a rhythm.
pub const MIN_SENTENCES_FOR_BURSTINESS: usize = 8;

/// Population standard deviation of sentence length (in words) below which the
/// rhythm reads as machine-flat. Human prose in a cover letter comfortably
/// clears this; a model producing uniformly 18-word sentences does not.
pub const MIN_SENTENCE_LENGTH_STDDEV: f64 = 4.0;

/// "X, Y, and Z" triplets tolerated per ten sentences before the rule-of-three
/// habit becomes the document's shape.
pub const MAX_TRIPLETS_PER_TEN_SENTENCES: f64 = 2.0;

/// One clause dash is allowed per this many words.
pub const EM_DASH_WORDS_PER_ALLOWED: usize = 150;

/// How much of a letter is scanned for a stock opener.
pub const TEMPLATE_OPENER_SCAN_CHARS: usize = 200;

/// Distinct posting keywords a cover letter must carry before it reads as
/// written for THIS role. Two, not one: the company name alone routinely
/// overlaps the posting's vocabulary, and a letter that only name-drops the
/// employer is exactly the generic letter this catches.
pub const MIN_JOB_SPECIFIC_TOKENS_IN_LETTER: usize = 2;

/// Conjunctions that close a "X, Y, and Z" triplet, per supported language.
const TRIPLET_CONJUNCTIONS: &[&str] = &[", and ", ", und ", ", or ", ", oder ", ", sowie "];

/// German adjective/participle endings the [`lexicon`]'s DE entries may carry.
///
/// The lexicon is GENERATED from `natural-voice.ts` and stores uninflected
/// stems ("nahtlos", "robust", "maßgeschneidert"), while German writes the
/// declined form ("nahtlose Integration", "robuste Systeme", "maßgeschneiderte
/// Lösungen"). [`contains_phrase`] requires a word boundary at both ends, so
/// the entire German half of the lexical check matched nothing on real output.
///
/// This is the full weak/strong declension paradigm and nothing else — a
/// bounded suffix, never an open-ended prefix match, so "roberta" still cannot
/// fire "robert".
const DE_INFLECTION_SUFFIXES: &[&str] = &["e", "em", "en", "er", "es"];

/// Fold the typographic apostrophe (U+2019) onto the ASCII one (U+0027).
///
/// [`flattened_lower`] normalizes case and whitespace but NOT punctuation, and
/// a model writes "it’s worth noting" about as often as "it's worth noting", so
/// a lexicon entry carrying an apostrophe would otherwise catch one spelling
/// and silently miss the other — the same dead-entry failure the German
/// inflection bug was. Folding at match time lets ONE ASCII entry cover both
/// (and makes the curly spelling unmatchable AS AN ENTRY, which the prompt-side
/// catalog-shape test pins).
///
/// Applied here rather than inside [`flattened_lower`] on purpose: that helper
/// is shared with `ats.rs`, where it normalizes résumé HEADER NAMES for an
/// equality comparison that has nothing to do with this lexicon. Widening a
/// shared normalizer for one caller's benefit is how the next false positive
/// gets in.
fn fold_apostrophes(text: &str) -> String {
    text.replace('\u{2019}', "'")
}

/// [`contains_phrase`], plus a bounded inflection suffix for German.
///
/// DE-only by construction: for every other language this is exactly
/// `contains_phrase`, so English keeps the both-ends word boundary it has
/// always had ("harnesses" does not fire the banned "harness"). Matching the
/// suffixed form through `contains_phrase` rather than loosening the boundary
/// rule keeps ONE definition of a word boundary in the pipeline.
fn contains_lexicon_phrase(haystack: &str, phrase: &str, lang: &str) -> bool {
    contains_phrase(haystack, phrase)
        || (lang == "de"
            && DE_INFLECTION_SUFFIXES
                .iter()
                .any(|suffix| contains_phrase(haystack, &format!("{phrase}{suffix}"))))
}

/// `voice.ai_tell_lexical` — a banned word or phrase the prompt told the model
/// not to use.
///
/// Both lexicon tiers feed this one code: the lexical list always, and the
/// prose-pattern list only for connected writing, where those rules apply. The
/// matched LEXICON ENTRY is the issue's evidence, so the user sees which ban
/// fired — for German that is the uninflected stem ("nahtlos") even when the
/// document wrote "nahtlose", because the entry is what the prompt banned and
/// the inflected form is still findable from it (see
/// [`contains_lexicon_phrase`]).
///
/// ## Scoped exactly like the prompt that issued the ban
///
/// `natural-voice.ts` scopes it in its first line — "Applies to any words YOU
/// introduce, never to exact job-ad keywords already grounded in the résumé" —
/// so a phrase the candidate's OWN résumé already uses is out of scope by
/// construction: the model did not introduce it, it carried it over. Flagging it
/// anyway made this check stricter than the prompt it exists to verify, and told
/// a finance candidate that their own "leverage calculator" was an AI tell.
///
/// The exemption is per-PHRASE, matched with the same word-boundary rule as the
/// hit itself: a source that says "leverage" exempts "leverage" and nothing
/// else, so every other entry on the list still fires in the same document.
fn ai_tell_issues(ctx: &Analysis, include_prose: bool) -> Vec<ContentIssue> {
    // Both sides folded, so the per-phrase source exemption survives a résumé
    // that spells the same contraction with the other apostrophe.
    let haystack = fold_apostrophes(&flattened_lower(ctx.input.generated));
    let source = fold_apostrophes(&flattened_lower(ctx.input.source_resume));
    let lists: Vec<&'static [&'static str]> = if include_prose {
        vec![
            lexicon::ai_tell_lexical(&ctx.lang),
            lexicon::ai_tell_prose(&ctx.lang),
        ]
    } else {
        vec![lexicon::ai_tell_lexical(&ctx.lang)]
    };
    let mut hits: Vec<&str> = lists
        .into_iter()
        .flatten()
        .copied()
        // Both sides go through the same matcher: an inflected form the
        // candidate's own résumé already uses must exempt the stem exactly as a
        // verbatim one does, or the DE tolerance would turn the source
        // exemption into a one-way ratchet.
        .filter(|phrase| contains_lexicon_phrase(&haystack, phrase, &ctx.lang))
        .filter(|phrase| !contains_lexicon_phrase(&source, phrase, &ctx.lang))
        .collect();
    hits.sort_unstable();
    hits.dedup();
    hits.into_iter()
        .map(|phrase| {
            issue(
                VOICE_AI_TELL_LEXICAL,
                None,
                format!(
                    "\"{phrase}\" is on the AI-tell list the generator was told to avoid. Use \
                     the plain word for the real thing instead."
                ),
                Some(phrase.to_string()),
            )
        })
        .collect()
}

/// `voice.template_opener` — a letter that opens like every other letter.
///
/// Folded with [`fold_apostrophes`] for the same reason [`ai_tell_issues`] is,
/// and at the same kind of call-site-local scope: `TEMPLATE_OPENERS_EN` obeys
/// the one-directional apostrophe rule (U+0027 allowed, U+2019 banned) that the
/// prompt-side catalog-shape test asserts over ALL SIX lexicon arrays, and
/// without the fold here an entry such as "i'm excited to apply" would match
/// only the ASCII spelling while that shape rule claimed it was whole. German
/// openers carry no apostrophe, so this is a no-op for them.
fn template_opener_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let opening: String = fold_apostrophes(&flattened_lower(ctx.input.generated))
        .chars()
        .take(TEMPLATE_OPENER_SCAN_CHARS)
        .collect();
    lexicon::template_openers(&ctx.lang)
        .iter()
        .find(|opener| opening.contains(**opener))
        .map(|opener| {
            vec![issue(
                VOICE_TEMPLATE_OPENER,
                None,
                format!(
                    "The letter opens with \"{opener}\", the same first line thousands of other \
                     applications use. Open with something only you could write."
                ),
                Some((*opener).to_string()),
            )]
        })
        .unwrap_or_default()
}

/// Population standard deviation of sentence lengths, in words.
fn sentence_length_stddev(lengths: &[usize]) -> f64 {
    if lengths.is_empty() {
        return 0.0;
    }
    let n = lengths.len() as f64;
    let mean = lengths.iter().sum::<usize>() as f64 / n;
    let variance = lengths
        .iter()
        .map(|&l| (l as f64 - mean).powi(2))
        .sum::<f64>()
        / n;
    variance.sqrt()
}

/// `voice.low_burstiness` — every sentence the same length.
fn burstiness_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let lengths: Vec<usize> = sentences(ctx.input.generated)
        .iter()
        .map(|s| word_count(s))
        .filter(|n| *n > 0)
        .collect();
    if lengths.len() < MIN_SENTENCES_FOR_BURSTINESS {
        return Vec::new();
    }
    let stddev = sentence_length_stddev(&lengths);
    if stddev >= MIN_SENTENCE_LENGTH_STDDEV {
        return Vec::new();
    }
    vec![issue(
        VOICE_LOW_BURSTINESS,
        None,
        format!(
            "Sentence lengths barely vary (standard deviation {stddev:.1} words across \
             {} sentences). Flat, uniform rhythm is the strongest single AI tell — mix a short \
             sentence into the long ones.",
            lengths.len()
        ),
        Some(format!("stddev={stddev:.1}")),
    )]
}

/// `voice.rule_of_three_density` — ideas forced into groups of three.
fn rule_of_three_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let all = sentences(ctx.input.generated);
    if all.is_empty() {
        return Vec::new();
    }
    let triplets = all
        .iter()
        .filter(|s| {
            let lower = format!("{} ", s.to_lowercase());
            TRIPLET_CONJUNCTIONS.iter().any(|conj| {
                // A triplet needs a comma BEFORE the closing conjunction:
                // "X, Y, and Z", not "X and Y".
                lower.find(conj).is_some_and(|at| lower[..at].contains(','))
            })
        })
        .count();
    let per_ten = triplets as f64 * 10.0 / all.len() as f64;
    if per_ten <= MAX_TRIPLETS_PER_TEN_SENTENCES {
        return Vec::new();
    }
    vec![issue(
        VOICE_RULE_OF_THREE_DENSITY,
        None,
        format!(
            "{triplets} of {} sentences are \"X, Y, and Z\" triplets. Breaking the pattern once \
             or twice makes the writing sound like a person.",
            all.len()
        ),
        Some(format!("{per_ten:.1} per 10 sentences")),
    )]
}

/// `voice.em_dash_overuse` — the single most-reported AI punctuation tell.
///
/// Counts em and en dashes used between clauses only: a tight numeric range
/// (`2020–2023`) is correct typography and is skipped.
fn em_dash_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let text = ctx.input.generated;
    let chars: Vec<char> = text.chars().collect();
    let clause_dashes = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, '—' | '–'))
        .filter(|(i, _)| {
            let tight_range = i
                .checked_sub(1)
                .and_then(|p| chars.get(p))
                .is_some_and(|c| c.is_ascii_digit())
                && chars.get(i + 1).is_some_and(char::is_ascii_digit);
            !tight_range
        })
        .count();
    let words = word_count(text);
    let allowed = words / EM_DASH_WORDS_PER_ALLOWED;
    if clause_dashes <= allowed {
        return Vec::new();
    }
    vec![issue(
        VOICE_EM_DASH_OVERUSE,
        None,
        format!(
            "{clause_dashes} long dashes across {words} words (about one per \
             {EM_DASH_WORDS_PER_ALLOWED} words is natural). Replace most with a period, comma, \
             colon, or parentheses."
        ),
        Some(format!("{clause_dashes} dashes / {words} words")),
    )]
}

/// `voice.generic_letter` — a letter that could have been sent to anyone.
///
/// Measured as distinct posting keywords the letter carries. Nothing about the
/// role, the product, or the stack means the reader learns nothing they could
/// not have guessed.
///
/// This is a POSTING COMPARISON, so it obeys the same
/// [`Analysis::posting_comparable`] gate `alignment::validate` does: nothing
/// extractable on the posting side, or an output that is not in the target
/// language, and it goes quiet. Counting an English ad's keywords in a German
/// letter yields ~0 every time — a guaranteed "this letter is generic" warning
/// stacked under the one finding that actually explains it, which is exactly the
/// cascade the language Critical suppresses everywhere else.
///
/// It ALSO obeys [`Analysis::posting_language_diverges`], which the coverage
/// checks do not, because this is the one posting comparison that INTERSECTS
/// the ad's vocabulary with a document's instead of comparing two documents'
/// coverage of the ad against each other. A German-language letter answering an
/// English-language ad — the ordinary DACH case, where nothing is wrong with
/// either text — scores ~0 on that intersection for reasons that have nothing
/// to do with how specific the letter is. See that method for why the premise
/// is scoped to this check rather than folded into `posting_comparable`.
///
/// *Accepted cost, stated:* a letter that IS generic goes unreported whenever
/// the ad's language diverges. This family is advisory (a Warning) and a wrong
/// warning costs more than a missed one, which is the trade every check in this
/// module makes.
fn generic_letter_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !ctx.posting_comparable() || ctx.posting_language_diverges() {
        return Vec::new(); // Nothing to be specific ABOUT, or not comparable.
    }
    let specific = ctx
        .generated_keywords
        .intersection(&ctx.job_keywords)
        .count();
    if specific >= MIN_JOB_SPECIFIC_TOKENS_IN_LETTER {
        return Vec::new();
    }
    vec![issue(
        VOICE_GENERIC_LETTER,
        None,
        format!(
            "This letter mentions {specific} thing(s) specific to the posting. Name the product, \
             the stack, or the problem in the ad — otherwise it reads as a template."
        ),
        Some(format!("{specific} posting-specific terms")),
    )]
}

/// Résumé voice checks: lexical bans only. Prose-flow rules (em-dash, rhythm,
/// rule-of-three) do not apply to bullets, which are deliberately terse and
/// same-shaped by ATS convention — running them here would flag every
/// well-formed résumé.
pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    ai_tell_issues(ctx, false)
}

/// Cover-letter voice checks: the lexical bans plus every prose-flow rule.
pub(super) fn validate_letter(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = ai_tell_issues(ctx, true);
    issues.extend(template_opener_issues(ctx));
    issues.extend(burstiness_issues(ctx));
    issues.extend(rule_of_three_issues(ctx));
    issues.extend(em_dash_issues(ctx));
    issues.extend(generic_letter_issues(ctx));
    issues
}
