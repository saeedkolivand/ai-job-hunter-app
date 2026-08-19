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
mod tests {
    use super::*;

    // -- real-world text shape: line-wrap + quote-prefix + NBSP -----------
    //
    // Every OTHER fixture in this file is artificially newline-free (a Rust
    // `\`-continuation strips both the newline AND the leading indent into
    // one long line) -- realistic in length while dodging the only property
    // that actually matters for a substring match: whether the phrase stays
    // CONTIGUOUS. Real mail hard-wraps at ~72-80 columns, and a quoted reply
    // region prefixes each wrapped line with "> ". These tests build a
    // genuinely wrapped, genuinely quoted body (real `\n`, not a literal) at
    // several measured widths, so a wrap boundary landing inside the
    // discriminating phrase is reproduced, not assumed.

    /// Word-wrap `text` to at most `width` columns, breaking only at
    /// existing spaces (never mid-word) -- mirrors how a real mail client
    /// hard-wraps a plain-text body -- then prefixes every line with "> "
    /// (one level of reply-quoting). A tiny test-only generator so the SAME
    /// source paragraph is rendered at several realistic widths instead of
    /// hand-typing wrapped text, which would silently depend on exactly the
    /// column boundary a bug depends on.
    fn wrap_quoted(text: &str, width: usize) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate_len = if line.is_empty() {
                word.chars().count()
            } else {
                line.chars().count() + 1 + word.chars().count()
            };
            if !line.is_empty() && candidate_len > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
            .into_iter()
            .map(|l| format!("> {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The source paragraph every wrap-width test below renders -- a
    /// realistic rejection body carrying the discriminating phrase, with
    /// ONE regular space swapped for U+00A0 (NBSP) between "we" and "have"
    /// (`mail-parser`'s `html_to_text` emits `&nbsp;` verbatim).
    fn nbsp_rejection_paragraph() -> String {
        "Thank you for your interest in our team here at Acme Corp. After \
         further discussion among the panel, we\u{00A0}have decided not be \
         moving forward with your application at this time, though we were \
         impressed by your background. We wish you the very best in your \
         ongoing search."
            .to_string()
    }

    #[test]
    fn wrapped_quoted_nbsp_body_at_72_cols_classifies_as_rejection() {
        let body = wrap_quoted(&nbsp_rejection_paragraph(), 72);
        assert!(body.contains('\n'), "fixture must be genuinely multi-line");
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn wrapped_quoted_nbsp_body_at_76_cols_classifies_as_rejection() {
        let body = wrap_quoted(&nbsp_rejection_paragraph(), 76);
        assert!(body.contains('\n'));
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn wrapped_quoted_nbsp_body_at_78_cols_classifies_as_rejection() {
        let body = wrap_quoted(&nbsp_rejection_paragraph(), 78);
        assert!(body.contains('\n'));
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn wrapped_quoted_nbsp_body_at_80_cols_classifies_as_rejection() {
        let body = wrap_quoted(&nbsp_rejection_paragraph(), 80);
        assert!(body.contains('\n'));
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn a_curly_apostrophe_still_matches_the_ascii_apostrophe_phrase() {
        // The corpus stores a straight apostrophe (') in e.g. the French
        // rejection phrase "n'a pas ete retenue"; real mail commonly sends
        // U+2019 (') instead. Body only (that phrase is Location::Body).
        //
        // Deliberately isolated to ONLY this phrase (no surrounding
        // boilerplate like "regrettons de vous informer", itself a
        // discriminating rejection phrase) -- an earlier version of this
        // test accidentally also matched that OTHER phrase and stayed green
        // with no apostrophe folding at all, proving nothing about the
        // apostrophe itself.
        let body = "Votre candidature n\u{2019}a pas \u{e9}t\u{e9} retenue.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn nfd_decomposed_accents_still_match_the_nfc_written_phrase() {
        // The corpus phrase is written NFC (precomposed "é" = U+00E9). Some
        // mail clients / OS text layers instead emit NFD (decomposed: "e"
        // U+0065 + COMBINING ACUTE ACCENT U+0301) — a naive substring match
        // against a decomposed body misses every accented phrase (measured:
        // 16 of 16 accented discriminating rejection phrases). Spelled out
        // with explicit `\u{}` escapes for "été" (not a literal decomposed
        // character pasted into the source) so the decomposition is
        // unambiguous and can't silently re-compose under an editor's own
        // normalization. Same isolation lesson as the apostrophe test above
        // — no other discriminating phrase nearby.
        let body = "Votre candidature n'a pas e\u{0301}te\u{0301} retenue.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    // ── per-language, per-intent: one discriminating phrase each, isolated
    // into its own test so a mutation that only breaks ONE case can never
    // hide behind an earlier case's `assert!` short-circuiting a shared loop.

    #[test]
    fn classifies_en_confirmation() {
        assert_eq!(
            classify_intent("Update", Some("if you are among qualified candidates")),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_en_interview() {
        assert_eq!(
            classify_intent("Update", Some("invite you for a job interview")),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_en_offer() {
        assert_eq!(
            classify_intent("Update", Some("having you on our team")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_en_rejection() {
        assert_eq!(
            classify_intent("Update", Some("move forward with other candidates")),
            Some(EmailIntent::Rejection)
        );
    }

    // German has NO discriminating confirmation phrase in the surviving
    // corpus (`"wir haben ihre bewerbung"` is the only `de` confirmation
    // entry and it is `discriminating: false`) — a real, documented gap, not
    // an oversight. See `de_confirmation_phrase_alone_is_not_enough_to_decide`
    // below, which pins exactly this. NOT a German-only gap, either — see
    // the module doc's "Recall gap, by intent, not just by language"
    // section: English itself has only ONE discriminating confirmation
    // phrase and ONE discriminating offer phrase.

    #[test]
    fn classifies_de_interview() {
        assert_eq!(
            classify_intent("Update", Some("virtuellen vorstellungsgespräch")),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_de_offer() {
        assert_eq!(
            classify_intent("Update", Some("zusage zu ihrer bewerbung")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_de_rejection() {
        assert_eq!(
            classify_intent("Update", Some("andere besetzung")),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn classifies_fr_confirmation() {
        assert_eq!(
            classify_intent("Update", Some("bonne réception de ta candidature")),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_fr_interview() {
        // Subject-only phrase — placed in the subject, not the body.
        assert_eq!(
            classify_intent("convocation à un entretien", None),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_fr_offer() {
        // Subject-only phrase.
        assert_eq!(
            classify_intent("acceptation de candidature", None),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_fr_rejection() {
        assert_eq!(
            classify_intent("Update", Some("ne donnerons pas suite")),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn classifies_es_confirmation() {
        // Subject-only phrase.
        assert_eq!(
            classify_intent("currículum recibido", None),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_es_interview() {
        // Subject-only phrase.
        assert_eq!(
            classify_intent("invitación para una entrevista", None),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_es_offer() {
        assert_eq!(
            classify_intent("Update", Some("entusiasmados de ofrecerte")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_es_rejection() {
        assert_eq!(
            classify_intent("Update", Some("no proceder con tu candidatura")),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn classifies_it_confirmation() {
        assert_eq!(
            classify_intent("Update", Some("la sua candidatura è arrivata")),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_it_interview() {
        // Subject-only phrase.
        assert_eq!(
            classify_intent("convocazione a colloquio", None),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_it_offer() {
        assert_eq!(
            classify_intent("Update", Some("lettera di impegno all'assunzione")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_it_rejection() {
        assert_eq!(
            classify_intent("Update", Some("non è risultata prescelta")),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn classifies_nl_confirmation() {
        assert_eq!(
            classify_intent("Update", Some("sollicitatie is binnen")),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_nl_interview() {
        assert_eq!(
            classify_intent("Update", Some("telefonisch gesprek van 30 minuten")),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_nl_offer() {
        assert_eq!(
            classify_intent("Update", Some("bieden je graag de functie")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_nl_rejection() {
        assert_eq!(
            classify_intent("Update", Some("niet verder mee te nemen")),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn classifies_pt_confirmation() {
        // Subject-only phrase.
        assert_eq!(
            classify_intent("confirmação de candidatura", None),
            Some(EmailIntent::Confirmation)
        );
    }

    #[test]
    fn classifies_pt_interview() {
        assert_eq!(
            classify_intent("Update", Some("queremos marcar uma entrevista")),
            Some(EmailIntent::Interview)
        );
    }

    #[test]
    fn classifies_pt_offer() {
        assert_eq!(
            classify_intent("Update", Some("parabéns pela proposta")),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn classifies_pt_rejection() {
        assert_eq!(
            classify_intent("Update", Some("não seguiremos o processo com você")),
            Some(EmailIntent::Rejection)
        );
    }

    // ── rule 3: a non-discriminating phrase never decides an intent alone ──

    #[test]
    fn en_non_discriminating_confirmation_phrase_alone_decides_nothing() {
        assert_eq!(
            classify_intent("Update", Some("you will hear from us")),
            None
        );
    }

    #[test]
    fn de_confirmation_phrase_alone_is_not_enough_to_decide() {
        // The only `de` confirmation entry in the surviving corpus is
        // `discriminating: false` — this is exactly why: it must not decide
        // anything on its own, so German confirmations currently classify
        // as `None` rather than `Some(Confirmation)`. A real, documented
        // corpus gap (see the module-level doc), not a bug in this rule.
        assert_eq!(
            classify_intent("Update", Some("wir haben ihre bewerbung")),
            None
        );
    }

    // ── rule 5: `location` is a constraint, not a hint ──────────────────────

    #[test]
    fn subject_only_phrase_does_not_fire_from_the_body() {
        // "convocation à un entretien" is Location::Subject.
        assert_eq!(
            classify_intent("Update", Some("convocation à un entretien")),
            None
        );
    }

    #[test]
    fn body_only_phrase_does_not_fire_from_the_subject() {
        // "move forward with other candidates" is Location::Body.
        assert_eq!(
            classify_intent("move forward with other candidates", None),
            None
        );
    }

    #[test]
    fn both_location_phrase_fires_from_the_subject_alone() {
        assert_eq!(
            classify_intent("not be moving forward with your application", None),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn both_location_phrase_fires_from_the_body_alone() {
        assert_eq!(
            classify_intent(
                "Update",
                Some("not be moving forward with your application")
            ),
            Some(EmailIntent::Rejection)
        );
    }

    // ── rule 1: negation lives INSIDE the phrase, never inferred ────────────

    #[test]
    fn positive_phrasing_without_the_negation_does_not_trigger_rejection() {
        // "moving forward with your application" alone (no "not") must not
        // fire the "not be moving forward with your application" rejection
        // phrase — a substring match against the FULL negated phrase can't
        // be fooled by its own positive tail.
        assert_eq!(
            classify_intent(
                "Next steps",
                Some("we are excited that you'll be moving forward with your application")
            ),
            None
        );
    }

    // ── rule 4: rejection wins whenever it fires alongside anything else ───

    #[test]
    fn rejection_wins_when_a_confirmation_phrase_and_a_rejection_phrase_both_fire() {
        // The exact real-world thread-reuse scenario this whole slice exists
        // for: a rejection whose body still carries an earlier confirmation
        // line (quoted reply, or a template that opens with a receipt
        // acknowledgement before the bad news).
        let body = "if you are among qualified candidates we will follow up. unfortunately, \
                     we have decided not be moving forward with your application at this time.";
        assert_eq!(
            classify_intent("Your application to Acme Corp", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn rejection_wins_over_interview_when_both_fire() {
        let body = "invite you for a job interview — actually, we have decided to move forward \
                     with other candidates instead.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn rejection_wins_over_offer_when_both_fire() {
        let body = "having you on our team would have been great, but we have decided to move \
                     forward with other candidates.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    // ── ladder tie-break among the 3 non-rejection intents ──────────────────

    #[test]
    fn offer_beats_interview_when_both_fire_without_rejection() {
        let body = "having you on our team — following up on your invite you for a job interview \
                     last week.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Offer)
        );
    }

    #[test]
    fn interview_beats_confirmation_when_both_fire_without_rejection() {
        let body =
            "if you are among qualified candidates, and — good news — we'd like to invite you \
             for a job interview.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Interview)
        );
    }

    // ── totality / bounded input ────────────────────────────────────────────

    #[test]
    fn empty_subject_and_no_body_classifies_as_no_intent_without_panicking() {
        assert_eq!(classify_intent("", None), None);
    }

    #[test]
    fn a_discriminating_phrase_past_intent_scan_bytes_is_still_not_matched() {
        // A cap still exists — just a far more generous one than the
        // fingerprint snippet. The padding length is a PINNED LITERAL, not
        // `INTENT_SCAN_BYTES` itself: deriving the padding from the same
        // constant it is meant to test makes the test self-referential — a
        // regression that bumps the constant to (say) 10 MB would bump this
        // padding to match and stay green, proving the cap exists while the
        // cap itself silently grew unbounded. A fixed 20_000 catches that: if
        // `INTENT_SCAN_BYTES` is ever raised, this phrase (just past the
        // OLD, pinned bound) becomes visible again and the assertion fails.
        //
        // This literal must be updated by hand if `INTENT_SCAN_BYTES` is
        // ever deliberately raised — that friction is the point.
        const PINNED_SCAN_BOUND_BYTES: usize = 20_000;
        let padding = "x".repeat(PINNED_SCAN_BOUND_BYTES);
        let body = format!("{padding} move forward with other candidates");
        assert_eq!(classify_intent("Update", Some(&body)), None);
    }

    #[test]
    fn a_realistic_rejection_body_past_the_old_500_byte_mark_classifies_as_rejection() {
        // A realistic full ATS rejection: a greeting plus "thank you for
        // applying"/volume-of-applicants boilerplate pushes the actual
        // discriminating phrase well past the OLD 500-byte
        // `BODY_SNIPPET_BYTES` mark — exactly the shape that was silently
        // missed before `INTENT_SCAN_BYTES` replaced it as this module's
        // body-scan bound.
        let body = "Dear Applicant,\n\n\
            Thank you so much for taking the time to apply for the Senior Backend Engineer \
            position at Acme Corp, and for your patience throughout our review process. We \
            received a very large number of applications for this role, and our hiring team \
            carefully reviewed every candidate's background, skills, and experience against \
            what the position required. This was one of the most competitive searches we have \
            run this year, and choosing among so many strong candidates was genuinely difficult \
            for the whole panel.\n\n\
            After careful consideration, we have decided not be moving forward with your \
            application at this time.\n\n\
            We will keep your resume on file for six months in case a better-matching role \
            opens up, and we wish you the very best in your job search. Thank you again for \
            your interest in Acme Corp.\n\n\
            Best regards,\nThe Acme Corp Talent Acquisition Team";
        assert!(
            body.len() > 500,
            "test fixture must actually exceed the old 500-byte cap to be meaningful"
        );
        assert_eq!(
            classify_intent("Your application to Acme Corp", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn unrelated_text_returns_no_intent() {
        assert_eq!(
            classify_intent(
                "Your weekly newsletter",
                Some("Enjoy this week's roundup of articles.")
            ),
            None
        );
    }

    // ── wider body window: reasoning about what a bigger scan lets in ───────

    #[test]
    fn rejection_still_wins_when_a_stale_quoted_confirmation_phrase_sits_past_the_old_cap() {
        // The exact concern a wider window raises: MORE quoted, earlier
        // thread content is now visible. Here a confirmation phrase from an
        // earlier message in the thread sits well past the OLD 500-byte cap
        // (which would previously have hidden it entirely). Rejection still
        // wins: the priority check has no positional/order dependence — it
        // just asks "does ANY rejection phrase match anywhere" — independent
        // of where in the (now wider) window a competing intent's phrase
        // also happens to sit.
        let padding = "quoted earlier thread content ".repeat(20);
        assert!(padding.len() > 500);
        let body = format!(
            "we have decided not be moving forward with your application. {padding} \
             if you are among qualified candidates we will be in touch."
        );
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Rejection)
        );
    }

    #[test]
    fn known_precision_limit_a_stale_quoted_offer_phrase_can_beat_a_current_interview_phrase() {
        // Documents a REAL, accepted-not-fixed limitation the wider window
        // introduces — unlike rejection (which always wins regardless of
        // position), the ladder tie-break among the non-rejection three
        // (`Offer` > `Interview` > `Confirmation`) has no positional
        // awareness either. A stale "having you on our team" offer line
        // quoted from an OLDER message in the thread — now visible because
        // the window is wider — outranks a genuinely CURRENT
        // interview-scheduling phrase, even though the offer phrase isn't
        // about the current email at all. Flagged for the coordinator, not
        // silently accepted or fixed in this slice.
        let padding = "quoted earlier thread content ".repeat(20);
        assert!(padding.len() > 500);
        let body = format!(
            "invite you for a job interview next Tuesday. {padding} having you on our team \
             would have been great."
        );
        assert_eq!(
            classify_intent("Update", Some(&body)),
            Some(EmailIntent::Offer)
        );
    }

    // ── rule 2: cross-language collisions don't matter (no language check) ──

    #[test]
    fn mixed_language_rejection_phrases_still_classify_as_rejection() {
        let body = "move forward with other candidates -- andere besetzung.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    // ── corpus shape: pins the count against silent drift ──────────────────

    #[test]
    fn corpus_shape_matches_the_173_phrase_survived_corpus() {
        // Reads `lang` straight out of the raw JSON (not via `PhraseEntry`,
        // which doesn't map that key — see its doc) so a future edit to
        // `intent_phrases.json` can't silently drop entries unnoticed.
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("intent_phrases.json")).expect("valid JSON");
        let entries = raw.as_array().expect("top-level JSON array");
        assert_eq!(entries.len(), 173);
        assert_eq!(PHRASES.len(), 173);
        let per_lang = |lang: &str| entries.iter().filter(|e| e["lang"] == lang).count();
        assert_eq!(per_lang("en"), 30);
        assert_eq!(per_lang("de"), 27);
        assert_eq!(per_lang("fr"), 22);
        assert_eq!(per_lang("es"), 30);
        assert_eq!(per_lang("it"), 19);
        assert_eq!(per_lang("nl"), 17);
        assert_eq!(per_lang("pt"), 28);
    }

    #[test]
    fn corpus_shape_pins_discriminating_counts_per_intent() {
        // `discriminating` is the SINGLE field gating whether a phrase can
        // decide an intent alone (see `PhraseEntry::discriminating`'s doc)
        // — flipping it on any one of ~114 non-discriminating entries would
        // silently change classifier behavior without touching a phrase's
        // text, its location, or any per-language total the previous test
        // already pins. Counted from `PHRASES` (post-parse, post-`fold`),
        // so this also exercises that the field actually deserializes.
        let count = |intent: EmailIntent| {
            PHRASES
                .iter()
                .filter(|p| p.intent == intent && p.discriminating)
                .count()
        };
        assert_eq!(count(EmailIntent::Confirmation), 9);
        assert_eq!(count(EmailIntent::Rejection), 76);
        assert_eq!(count(EmailIntent::Interview), 30);
        assert_eq!(count(EmailIntent::Offer), 23);
        assert_eq!(
            PHRASES.iter().filter(|p| p.discriminating).count(),
            9 + 76 + 30 + 23,
            "must equal the 138 total discriminating entries"
        );
    }

    #[test]
    fn corpus_content_every_phrase_is_non_empty_and_already_lowercase() {
        for entry in PHRASES.iter() {
            assert!(
                !entry.phrase.trim().is_empty(),
                "an empty phrase can never usefully match anything — must be a data bug"
            );
            assert_eq!(
                entry.phrase,
                entry.phrase.to_lowercase(),
                "phrase {:?} is not already lower-cased — matching relies on this",
                entry.phrase
            );
        }
    }

    #[test]
    fn corpus_content_every_discriminating_phrase_is_at_least_10_bytes() {
        // A short discriminating phrase can fire on ordinary, unrelated
        // mail — and per the write-path gating this classifier feeds, a
        // false rejection auto-writes an absorbing `Rejected`. 10 bytes is
        // the shortest phrase that survived the corpus's own adversarial
        // pass ("werturteil") — this pins that floor so a future addition
        // can't slip under it unnoticed.
        for entry in PHRASES.iter().filter(|p| p.discriminating) {
            assert!(
                entry.phrase.len() >= 10,
                "discriminating phrase {:?} is under 10 bytes — too short to \
                 safely decide an intent alone",
                entry.phrase
            );
        }
    }
}
