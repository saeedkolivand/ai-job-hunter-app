//! Pure 4-way email-intent classification (confirmation | rejection |
//! interview | offer) — the body-level signal [`parser`](super::parser)'s
//! subject-only fingerprint gate cannot see. A real ATS rejection commonly
//! reuses the confirmation subject line from earlier in the thread (see the
//! `known_false_positive_*` tests in [`super::parser`]), so the
//! discriminating signal has to come from the BODY, not the subject.
//!
//! **Corpus.** `intent_phrases.json` (sibling file, in this same directory)
//! is a slimmed, compiled-in asset (`include_str!`'d, parsed once behind
//! [`PHRASES`]) derived from a 173-phrase, 7-language (en/de/fr/es/it/nl/pt)
//! corpus that survived a multi-agent adversarial pass (57% kill rate on
//! proposed phrases — every phrase here already beat that bar). Only the
//! 5 fields the classifier needs survive the trim (`lang`, `intent`,
//! `phrase`, `location`, `discriminating`) — the corpus's `evidence`/
//! `killed`/`ambiguousByLang`/`notes`/`verdicts` blocks are stripped. Each
//! phrase already has any required negation baked directly into its text —
//! e.g. the rejection phrase is `"not be moving forward with your
//! application"`, never the bare `"moving forward with your application"`
//! (an interview signal). Classification never infers or strips negation; it
//! only ever does a verbatim, case-folded substring match against a fixed
//! phrase.
//!
//! A JSON asset (rather than a generated `.rs` table) was chosen because
//! `cargo fmt` explodes a 173-entry struct-literal array onto ~5 lines each
//! — pushing this module well past the architecture test's R8 hard LOC cap
//! (`tests/architecture.rs`). A data file sidesteps that entirely and keeps
//! the corpus a one-line-per-entry diff.
//!
//! **Recall gap, by intent, not just by language.** The 138 discriminating
//! phrases split 9 confirmation / 76 rejection / 30 interview / 23 offer
//! (pinned by `corpus_shape_pins_discriminating_counts_per_intent`) — the
//! adversarial pass killed far more confirmation/offer wording than
//! rejection wording (a rejection has many stock templates; a confirmation
//! is often one bland sentence). This is NOT only a non-English problem:
//! **English itself has exactly ONE discriminating confirmation phrase and
//! ONE discriminating offer phrase**, against 23 English rejection
//! phrases — the primary language's confirmation/offer recall is nearly as
//! thin as German's (which has ZERO discriminating confirmation phrases at
//! all — see `de_confirmation_phrase_alone_is_not_enough_to_decide` below).
//! `classify_intent` returning `None` for a genuine confirmation/offer
//! email is the SAFE direction (no write), not a crash or a wrong write —
//! but it means recall on those two intents is real-world thinner than the
//! phrase count alone suggests.
//!
//! **Language is not used to decide intent.** Cross-language phrase
//! collisions don't matter — the classifier only needs the intent, not the
//! language — so `lang` is kept on each entry for human auditability only;
//! this module never reads it to decide anything, and never calls language
//! detection.
//!
//! **Privacy.** Same guarantee as [`super::parser`]: nothing here logs
//! subject/body content, and the only thing a caller ever gets back is an
//! [`EmailIntent`] variant — never the matched text.
//!
//! **Body scan bound: [`INTENT_SCAN_BYTES`], deliberately its own constant —
//! not [`super::parser::BODY_SNIPPET_BYTES`].** That 500-byte constant sizes
//! a cheap first-pass FINGERPRINT snippet ("does this look like an
//! application email at all") — a different job with a different cost of
//! being wrong. A real ATS rejection routinely opens with a greeting and
//! "thank you for applying" boilerplate before its discriminating phrase, so
//! that phrase commonly sits well past 500 bytes; missing it is the single
//! failure this whole slice exists to prevent (auto-marking a REJECTED
//! application as merely confirmed — see
//! `known_false_positive_a_rejection_email_still_fingerprints` in
//! [`super::parser`], and this module's own
//! `a_realistic_rejection_body_past_the_old_500_byte_mark_classifies_as_rejection`
//! test). `INTENT_SCAN_BYTES` is NOT a reuse of an upstream fetch bound
//! either — [`crate::email_watch::imap_client::MAX_BODY_BYTES`] (200,000
//! bytes) already caps the raw fetch at the IMAP protocol level, and
//! `poller::run_tick` re-applies that same cap defensively post-fetch, but a
//! byte cap is not a memory cap if anything decompresses before it reaches
//! here — so this module keeps its own independent, generous-but-real bound
//! rather than trusting an upstream cap it cannot see or verify at this call
//! site.
//!
//! Both functions in this module are pure and total (never panic on
//! malformed/hostile input) and have no IMAP/Tauri/network coupling.
//! Neither one writes anything: [`classify_intent`] only decides an intent,
//! and [`next_status`] only decides what a *would-be* write should be —
//! wiring an actual [`crate::applications::ApplicationStore`] write from a
//! poller tick is a later slice. [`PHRASES`] itself is a compiled-in build
//! asset validated by this module's own tests, not runtime input — a
//! corrupted `intent_phrases.json` is a build-time bug (caught immediately
//! by any test run), the same posture [`super::parser::SUBJECT_PATTERNS`]
//! already takes for its own compiled-in regex literals.

use std::sync::LazyLock;

use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

use crate::email_watch::parser::{safe_prefix, SUBJECT_MAX_BYTES};

// The ladder rule lives in the sibling `status_ladder` module (split out to
// stay under R8's LOC cap) — re-exported here so `email_watch::intent::
// next_status`/`is_actionable` stay the paths every other caller/doc
// comment in this module family already uses.
pub(super) use crate::email_watch::status_ladder::is_actionable;
pub use crate::email_watch::status_ladder::next_status;

/// Fold real-mail text shape into the SAME normal form the corpus phrases
/// are already written in, so a substring match survives what a plain
/// `.to_lowercase()` alone does not (measured, independently, by two
/// review passes — see this module's `wrapped_quoted_nbsp_body_at_*` and
/// `a_curly_apostrophe_*` tests, each proven to fail before this fn existed):
///
/// - **Line-wrap + quote-prefix.** Real mail hard-wraps at ~72-80 columns,
///   and a quoted-reply region prefixes every wrapped line with `"> "` (or
///   `">> "` nested). A phrase split across a wrap boundary needs the
///   newline treated as a plain word-separating space — but naively doing
///   that alone glues a NEXT line's `"> "` marker into the middle of the
///   reconstructed phrase, so each line's leading quote markers are
///   stripped FIRST, before line joining.
/// - **Any Unicode whitespace, not just ASCII.** `char::is_whitespace()`
///   already covers U+00A0 NBSP (`mail-parser`'s `html_to_text` emits
///   `&nbsp;` verbatim), so no special-casing is needed for it once every
///   whitespace char (and every line-join point) folds to one space and
///   runs collapse.
/// - **Curly/modifier apostrophes.** `\u{2019}`/`\u{02BC}` fold to the
///   ASCII `'` the corpus phrases are written with (e.g. the French
///   rejection phrase `"n'a pas été retenue"`).
/// - **NFC composition.** A decomposed accented body (base char + combining
///   mark, e.g. from some mail clients / OS text layers) is composed back
///   to the single precomposed codepoint the corpus phrases use, via
///   `unicode-normalization` — already resolved transitively (typst/
///   pdf-extract/stringprep all pull it in), so this adds no new supply-
///   chain surface.
///
/// Applied identically to the haystack (subject/body, AFTER [`safe_prefix`]
/// so the byte bound still applies first) and to every [`PhraseEntry::
/// phrase`] needle (once, at [`PHRASES`] build time — they're static, so
/// folding them costs nothing per email).
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    let mut started = false;
    for raw_line in s.split('\n') {
        for ch in strip_quote_prefix(raw_line).chars() {
            if ch.is_whitespace() {
                if started {
                    pending_space = true;
                }
                continue;
            }
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(match ch {
                '\u{2019}' | '\u{02BC}' => '\'',
                other => other,
            });
            started = true;
        }
        // A line boundary is itself always at least one word-separating
        // space, exactly like any other whitespace run this fn collapses.
        if started {
            pending_space = true;
        }
    }
    out.nfc().collect::<String>().to_lowercase()
}

/// Strip a leading reply-quote marker (`"> "`, `">> "`, `"> > "`, …) from
/// one line, so joining wrapped-and-quoted lines back together in [`fold`]
/// doesn't glue a quote marker into the middle of a phrase that wrapped
/// across the line. A line with no leading `>` at all is returned
/// unchanged. (A body paragraph that happens to start a line with a bare
/// `>` for some OTHER reason loses that character — accepted: no corpus
/// phrase starts with or depends on a literal `>`.)
fn strip_quote_prefix(line: &str) -> &str {
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start_matches([' ', '\t']);
        match trimmed.strip_prefix('>') {
            Some(after) => rest = after,
            None => break,
        }
    }
    rest.trim_start_matches([' ', '\t'])
}

/// The 4-way intent this classifier decides between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmailIntent {
    Confirmation,
    Rejection,
    Interview,
    Offer,
}

/// Where a phrase is allowed to match — a constraint, not a hint: a `Body`
/// phrase must never be matched against a subject line, and vice versa for
/// `Subject` (mirrors the corpus's own `location` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Location {
    Subject,
    Body,
    Both,
}

/// `intent_phrases.json` also carries a `lang` key per entry (kept in the
/// FILE for human auditability — see the module doc's "Language is not used
/// to decide intent" note), but this struct deliberately doesn't map it:
/// `serde` ignores unknown JSON keys by default, and matching logic would
/// never read it anyway. The `corpus_shape` test below reads `lang` straight
/// out of the raw JSON (not through this struct) to pin the per-language
/// phrase counts, so a future edit to the file can't silently drop entries.
#[derive(Debug, Clone, Deserialize)]
struct PhraseEntry {
    intent: EmailIntent,
    /// Already lower-cased in the source corpus; matched via a verbatim
    /// substring check against a lower-cased subject/body — no regex, since
    /// every entry is a fixed phrase, not a pattern.
    phrase: String,
    location: Location,
    /// `true`: this phrase alone is enough to decide `intent`. `false`:
    /// supporting evidence only — [`classify_intent`] never lets a
    /// non-discriminating phrase decide anything by itself.
    discriminating: bool,
}

/// The compiled-in corpus: all 173 surviving phrases, parsed once from
/// `intent_phrases.json` on first use. 138 entries are `discriminating:
/// true`; the other 35 are non-discriminating and are compiled in for
/// completeness (a future confidence/scoring pass could weigh them), but
/// [`classify_intent`] never reads a non-discriminating entry when deciding
/// an intent — see `discriminating_hit` below, which filters to
/// `discriminating: true` before matching anything.
///
/// **Fails CLOSED, never aborts.** `.unwrap_or_default()`, not `.expect(…)`:
/// the release profile sets `panic = "abort"`, so a corrupted
/// `intent_phrases.json` must not take down the whole desktop process. An
/// empty `Vec` here makes [`discriminating_hit`] vacuously `false` for
/// every intent, so [`classify_intent`] always returns `None` — nothing
/// ever writes (`crate::email_watch::auto_write` only calls `next_status`
/// on a `Some` intent), which is exactly the safe direction for a corpus
/// that failed to parse. CI still catches real corruption: `corpus_shape`
/// below asserts the exact non-empty counts.
///
/// Each phrase is [`fold`]ed once here (not per email) — see `fold`'s own
/// doc for what that normalizes and why.
static PHRASES: LazyLock<Vec<PhraseEntry>> = LazyLock::new(|| {
    let mut entries: Vec<PhraseEntry> =
        serde_json::from_str(include_str!("intent_phrases.json")).unwrap_or_default();
    for entry in &mut entries {
        entry.phrase = fold(&entry.phrase);
    }
    entries
});

fn phrase_matches(entry: &PhraseEntry, subject: &str, body: &str) -> bool {
    let phrase = entry.phrase.as_str();
    match entry.location {
        Location::Subject => subject.contains(phrase),
        Location::Body => body.contains(phrase),
        Location::Both => subject.contains(phrase) || body.contains(phrase),
    }
}

/// Whether any `discriminating: true` phrase for `intent` matches — the
/// ONLY thing [`classify_intent`] ever consults to decide an intent, so a
/// non-discriminating entry can never single-handedly decide anything (see
/// [`PhraseEntry::discriminating`]'s doc).
fn discriminating_hit(intent: EmailIntent, subject: &str, body: &str) -> bool {
    PHRASES
        .iter()
        .any(|p| p.intent == intent && p.discriminating && phrase_matches(p, subject, body))
}

/// Bound on how many bytes of the body [`classify_intent`] scans for a
/// discriminating phrase — see the module doc's "Body scan bound" section
/// for why this is a deliberately separate, much larger constant than
/// [`super::parser::BODY_SNIPPET_BYTES`], not a reuse of it.
///
/// Sized to comfortably cover a realistic FULL ATS email — greeting,
/// "thank you for applying"/volume-of-applicants boilerplate, the actual
/// decision paragraph, a signature, and a legal/EEO footer — not a marginal
/// bump over the fingerprint snippet. `str::contains` (like `regex`) is a
/// linear, non-backtracking scan, so a generous bound here is nearly free —
/// but this is still a REAL bound, not `usize::MAX`: it guards against a
/// hostile/pathological multi-megabyte body reaching this module directly
/// (a byte cap is not a memory cap if anything decompresses before it).
const INTENT_SCAN_BYTES: usize = 20_000;

/// Decide the 4-way intent of one email from its subject and (optional)
/// body. `None` means no discriminating phrase matched anything — genuinely
/// ambiguous, e.g. a "finish your draft application" nudge that fingerprints
/// on subject alone (see [`super::parser::fingerprint`]) but carries none of
/// the 4 intents' body language.
///
/// The subject is bounded by [`SUBJECT_MAX_BYTES`] (reused from
/// [`super::parser`] — subject lines are always short, so this cap is not
/// the concern the body one is). The body is bounded by
/// [`INTENT_SCAN_BYTES`] — see this fn's doc above and the module doc's
/// "Body scan bound" section for why that is its own constant.
///
/// **A wider body window than the old 500-byte cap also means more of a
/// QUOTED, earlier thread message enters the scan** (an old confirmation
/// line further down in a rejection reply, or vice versa). Rejection-wins
/// (below) makes the confirmation/rejection version of that safe by
/// construction — see
/// `rejection_still_wins_when_a_stale_quoted_confirmation_phrase_sits_past_the_old_cap`.
/// It does NOT fully cover the non-rejection three: a stale quoted
/// higher-priority phrase (e.g. an old "having you on our team" offer line
/// quoted beneath a new, unrelated interview-scheduling email) can now be
/// seen where the 500-byte cap would previously have hidden it, and the
/// ladder tie-break below would pick the (stale) `Offer` over the (current)
/// `Interview` — see `known_precision_limit_a_stale_quoted_offer_phrase_can_beat_a_current_interview_phrase`,
/// which documents this as an accepted, unfixed limitation, not a bug.
///
/// **Rejection wins whenever it fires alongside any other intent** — a
/// deliberate asymmetry (missing a rejection costs far more than missing a
/// confirmation, an interview invite, or an offer): a real rejection reply
/// commonly still carries an earlier intent's wording in the same message
/// (quoted thread history, or a template that opens with a receipt
/// acknowledgement — or even an interview-scheduling line — before the bad
/// news).
///
/// Among the remaining three, ties break by ladder order (`Offer` >
/// `Interview` > `Confirmation`): the more-advanced-stage phrase is treated
/// as the rarer, more specific signal. The corpus had no naturally-occurring
/// dual-intent example among these three to measure this against — a
/// defensible default, flagged for review rather than silently assumed.
pub fn classify_intent(subject: &str, body: Option<&str>) -> Option<EmailIntent> {
    let subject = fold(safe_prefix(subject, SUBJECT_MAX_BYTES));
    let body = fold(safe_prefix(body.unwrap_or_default(), INTENT_SCAN_BYTES));

    if discriminating_hit(EmailIntent::Rejection, &subject, &body) {
        return Some(EmailIntent::Rejection);
    }
    [
        EmailIntent::Offer,
        EmailIntent::Interview,
        EmailIntent::Confirmation,
    ]
    .into_iter()
    .find(|&intent| discriminating_hit(intent, &subject, &body))
}

#[cfg(test)]
mod tests;
