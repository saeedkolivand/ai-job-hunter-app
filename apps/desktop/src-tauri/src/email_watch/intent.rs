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

use crate::applications::ApplicationStatus;
use crate::email_watch::parser::{safe_prefix, BODY_SNIPPET_BYTES, SUBJECT_MAX_BYTES};

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
static PHRASES: LazyLock<Vec<PhraseEntry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("intent_phrases.json"))
        .expect("intent_phrases.json is a compiled-in asset, valid at build time")
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

/// Decide the 4-way intent of one email from its subject and (optional)
/// body. `None` means no discriminating phrase matched anything — genuinely
/// ambiguous, e.g. a "finish your draft application" nudge that fingerprints
/// on subject alone (see [`super::parser::fingerprint`]) but carries none of
/// the 4 intents' body language.
///
/// Both fields are bounded the same way [`super::parser`] already bounds
/// them (`SUBJECT_MAX_BYTES`/`BODY_SNIPPET_BYTES`) rather than inventing a
/// new cap — see this module's doc for the known limitation that implies.
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
    let subject = safe_prefix(subject, SUBJECT_MAX_BYTES).to_lowercase();
    let body = safe_prefix(body.unwrap_or_default(), BODY_SNIPPET_BYTES).to_lowercase();

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

// ── The pure status-ladder rule (consumes an intent, decides on nothing yet) ──

/// Live (non-terminal) statuses this rule is allowed to move. The other 4
/// (`Accepted`/`Rejected`/`Ghosted`/`Withdrawn`) are final outcomes — see
/// [`next_status`]'s doc for why nothing moves them further.
fn is_live(status: ApplicationStatus) -> bool {
    matches!(
        status,
        ApplicationStatus::Saved
            | ApplicationStatus::Applied
            | ApplicationStatus::Screening
            | ApplicationStatus::Interviewing
            | ApplicationStatus::Offer
    )
}

/// Forward-ladder position for the 5 live statuses (higher = further along).
/// [`next_status`] only ever calls this on a status already known `is_live`,
/// but the match stays total (never panics) — a terminal status is parked at
/// `u8::MAX` so a future caller mistake could only ever look like "no
/// advance available" (fails closed), never manufacture a spurious advance.
fn ladder_rank(status: ApplicationStatus) -> u8 {
    match status {
        ApplicationStatus::Saved => 0,
        ApplicationStatus::Applied => 1,
        ApplicationStatus::Screening => 2,
        ApplicationStatus::Interviewing => 3,
        ApplicationStatus::Offer => 4,
        ApplicationStatus::Accepted
        | ApplicationStatus::Rejected
        | ApplicationStatus::Ghosted
        | ApplicationStatus::Withdrawn => u8::MAX,
    }
}

/// `target` only if it is strictly further along the ladder than `current`
/// — never a regression, and never a same-stage no-op write.
fn advance_to(target: ApplicationStatus, current: ApplicationStatus) -> Option<ApplicationStatus> {
    (ladder_rank(target) > ladder_rank(current)).then_some(target)
}

/// What one classified [`EmailIntent`] should do to `current`'s status.
/// `None` means no-op — don't write anything — never a downgrade.
///
/// - **Terminal statuses never move.** `Accepted`/`Rejected`/`Ghosted`/
///   `Withdrawn` are final outcomes. A later email (a resend, a
///   stale/reordered notification, or a genuinely new event this 4-way
///   classifier has no intent for, like a rescinded offer) is out of scope:
///   silently overwriting a final — usually user-confirmed — outcome would
///   be worse than requiring a human to handle that rare edge case.
/// - **Rejection wins from any LIVE status** (`Saved` through `Offer`),
///   including from `Offer` itself — an offer can be rescinded by the
///   employer before the candidate accepts it.
/// - **Everything else only ever advances, never regresses or repeats.** A
///   late confirmation arriving once the application is already at `Offer`
///   is a no-op, not a downgrade to `Applied`; an intent that maps to the
///   CURRENT status (e.g. a second interview-round email while already
///   `Interviewing`) is also a no-op — nothing changed, so nothing writes.
///   `Screening` is deliberately reachable only by direct status edits, not
///   by any of these 4 intents — an `Interview` intent jumps straight to
///   `Interviewing` even from `Saved`/`Applied`, which is a forward skip,
///   not a regression, and is allowed.
pub fn next_status(intent: EmailIntent, current: ApplicationStatus) -> Option<ApplicationStatus> {
    if !is_live(current) {
        return None;
    }
    match intent {
        EmailIntent::Rejection => Some(ApplicationStatus::Rejected),
        EmailIntent::Confirmation => advance_to(ApplicationStatus::Applied, current),
        EmailIntent::Interview => advance_to(ApplicationStatus::Interviewing, current),
        EmailIntent::Offer => advance_to(ApplicationStatus::Offer, current),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    // below, which pins exactly this.

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
    fn a_discriminating_phrase_past_the_body_snippet_cap_is_not_matched() {
        // Documents the real, current limitation: `classify_intent` bounds
        // its body scan to `BODY_SNIPPET_BYTES` (matching `super::parser`'s
        // existing cap rather than inventing a new one) — a discriminating
        // phrase that starts past that cutoff is invisible to it.
        let padding = "x".repeat(BODY_SNIPPET_BYTES);
        let body = format!("{padding} move forward with other candidates");
        assert_eq!(classify_intent("Update", Some(&body)), None);
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

    // ── rule 2: cross-language collisions don't matter (no language check) ──

    #[test]
    fn mixed_language_rejection_phrases_still_classify_as_rejection() {
        let body = "move forward with other candidates -- andere besetzung.";
        assert_eq!(
            classify_intent("Update", Some(body)),
            Some(EmailIntent::Rejection)
        );
    }

    // ── next_status: the pure ladder rule ───────────────────────────────────

    #[test]
    fn confirmation_from_saved_advances_to_applied() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Saved),
            Some(ApplicationStatus::Applied)
        );
    }

    #[test]
    fn confirmation_from_offer_is_a_noop_not_a_downgrade() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Offer),
            None
        );
    }

    #[test]
    fn confirmation_from_applied_is_a_noop_same_stage() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Applied),
            None
        );
    }

    #[test]
    fn interview_from_saved_skips_forward_to_interviewing() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Saved),
            Some(ApplicationStatus::Interviewing)
        );
    }

    #[test]
    fn interview_from_interviewing_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Interviewing),
            None
        );
    }

    #[test]
    fn interview_from_offer_is_a_noop_not_a_downgrade() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Offer),
            None
        );
    }

    #[test]
    fn offer_intent_from_saved_skips_forward_to_offer() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Saved),
            Some(ApplicationStatus::Offer)
        );
    }

    #[test]
    fn offer_intent_from_offer_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Offer),
            None
        );
    }

    #[test]
    fn rejection_from_saved_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Saved),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_applied_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Applied),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_screening_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Screening),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_interviewing_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Interviewing),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_offer_advances_to_rejected() {
        // An offer can be rescinded by the employer before the candidate
        // accepts it — rejection is allowed from `Offer` too.
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Offer),
            Some(ApplicationStatus::Rejected)
        );
    }

    // ── terminal statuses: nothing moves them, not even Rejection ──────────

    #[test]
    fn rejection_from_accepted_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Accepted),
            None
        );
    }

    #[test]
    fn rejection_from_already_rejected_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Rejected),
            None
        );
    }

    #[test]
    fn rejection_from_ghosted_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Ghosted),
            None
        );
    }

    #[test]
    fn rejection_from_withdrawn_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Withdrawn),
            None
        );
    }

    #[test]
    fn confirmation_from_accepted_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Accepted),
            None
        );
    }

    #[test]
    fn interview_from_already_rejected_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Rejected),
            None
        );
    }

    #[test]
    fn offer_intent_from_withdrawn_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Withdrawn),
            None
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
}
