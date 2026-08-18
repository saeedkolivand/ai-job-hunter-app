//! Hard-constraint pass: the non-negotiables a relevance score cannot carry.
//!
//! `combined = f(semantic, ats)` measures how much of a posting's vocabulary the
//! résumé covers. It is a good answer to "is this role about what I do?" and it
//! is structurally incapable of answering "could I actually take this job?" — so
//! a candidate who fails a non-negotiable still scores well on keyword overlap.
//! This module answers the second question **separately** and reports it as its
//! own payload field.
//!
//! ## Three rules, and why each exists
//!
//! 1. **Never touches the score.** The verdict is computed in the L3 command
//!    ([`super::match_resume`]) AFTER the kernel returns, and merged into the
//!    result value. It is not a multiplier, not a penalty, and not an input to
//!    [`super::score_one`] — which means it also never enters the `match_scores`
//!    cache. That is not just hygiene: the cache key is composed of SCORING
//!    inputs only, so a verdict cached under it would go stale the moment the
//!    user edits their preferences and would then be served for the whole TTL.
//! 2. **Never hides a job.** Nothing here filters, sorts, or gates. A wrong
//!    knock-out tells the user not to bother applying, which is far more costly
//!    than a wrong relevance number.
//! 3. **Absence of evidence is never evidence.** [`ConstraintStatus`] has FOUR
//!    values, and the two "we don't know" ones are distinct from both a pass and
//!    a fail. [`ConstraintStatus::NotMet`] requires positive evidence on BOTH
//!    sides — the posting states the thing, and the candidate's own stored data
//!    contradicts it.
//!
//! ## No shipped constraint emits `NotMet` today. That is the finding.
//!
//! The four constraints the architecture audit ranked, and what this app
//! actually holds for each:
//!
//! - **`workAuthorization`** — ranked first, and the app stores nothing about it
//!   anywhere: no visa/sponsorship/citizenship/right-to-work field on
//!   `ContactProfile` or `JobPreferences`. The tempting proxy — the candidate's
//!   own address — is exactly the false accusation: living in one country is not
//!   evidence of lacking authorization in another.
//! - **`employmentType`** — no persisted candidate preference exists.
//!   `AutopilotTarget::work_type` is a per-autopilot SEARCH filter (and is a
//!   work arrangement: remote/hybrid/on-site, not full-time/contract), scoped to
//!   one autopilot config; the Jobs-page match has no autopilot in hand.
//! - **`salaryFloor`** — `job_preferences.salary_expectation` is free text
//!   ("80k DOE"), documented as the answer to an application's
//!   salary-expectation question. An expectation is not a stated floor, it
//!   carries no currency and no period, and the posting side is a structured
//!   range on ONE board (Adzuna's `salaryMin`/`salaryMax`/`salaryCurrency`).
//!   Comparing them needs a free-text money parser plus an FX assumption — two
//!   invented facts per verdict.
//! - **`location`** — shipped, but **`Met` / `Unknown` / `NoPreference` only**.
//!   Two independent facts stop it short of a knock-out, and both were found by
//!   review after the first cut of this module did publish `NotMet`:
//!
//!   1. *The candidate side is a search-personalization setting, not a mobility
//!      statement.* `job_preferences.location` is written by one free-text
//!      "Preferred Location" input whose own description reads "personalize
//!      search results and recommendations". `JobsPage` seeds its scrape form
//!      from it ONE WAY and never writes back, so a user who deliberately
//!      searches Austin while Settings still says Berlin leaves no trace here.
//!      `PostingsCache` holds postings from every past search and retains no
//!      per-posting record of the `LocationSpec` that produced them, so this
//!      pass cannot recover which search a given posting came from.
//!   2. *A failed substring match is absence of evidence, not evidence of
//!      conflict.* [`location_verdict`]'s `Mismatch` fires on
//!      `Germany`/`Berlin` (granularity), `Vienna`/`Wien` and `NYC`/`New York`
//!      (exonym and abbreviation, outside the curated table), `Berlin`/`EMEA`
//!      and `Berlin`/`Multiple locations` (non-place location text), and
//!      `Berlin`/`Telecommute` (a remote synonym the marker list lacks). It
//!      would also fire on most postings from the app's primary aggregator:
//!      Adzuna never writes `extra.remote` (only the three remote-only boards
//!      do), so remote detection there rests entirely on location text Adzuna's
//!      `display_name` does not carry.
//!
//!   The same predicate is safe where it already lives — as the scrape-time
//!   post-filter, judging a posting against a location the user typed seconds
//!   earlier, costing one row in one search. It is not safe as a published
//!   verdict against every cached posting, keyed off a settings field. So the
//!   matcher is reused and the epistemics are re-derived: `Mismatch` maps to
//!   `Unknown` here.
//!
//! ### Why `Met` survives facts that killed `NotMet`
//!
//! Every fact above is direction-neutral, so the mirror case has to be answered
//! rather than assumed: the same stale `Berlin` setting, against the `Berlin`
//! rows still sitting in `PostingsCache` from an earlier search, would report
//! `met`. If that read as "this posting matches where you're looking" it would
//! be exactly as false as the knock-out was, from exactly the same field.
//!
//! The asymmetry is in what each status ASSERTS, not in how confident we feel:
//!
//! - `NotMet` asserted a CONFLICT — that the posting is somewhere the candidate
//!   will not go. Nothing in this app establishes that relation. A failed
//!   substring search is not a proof of non-membership, and no phrasing repairs
//!   it, because the underlying fact was never in hand.
//! - `Met` asserts a MATCH between two strings we hold in full. The relation —
//!   every token of the stored preference is present in the posting's location —
//!   is established deterministically from data we have, and stays true no
//!   matter which search produced the row.
//!
//! So `Met` is repaired by making the claim say what it is a claim ABOUT, not by
//! deleting it (which would leave a contract with no positive state and make the
//! whole pass vacuous). The claim is **"your stored Preferred Location matches
//! this posting"** — literally correct for any cached posting — and never "this
//! posting matches where you're looking", which imports a search intent this
//! pass cannot see. That is why the check's wire id is `preferredLocation`.
//!
//! On top of that, `Met` takes the STRICT reading of the matcher: the posting is
//! flagged or marked remote, or every place token the user actually typed
//! appears as a WHOLE token of the posting's location (diaeresis variants and
//! curated exonyms included). The looser reading the scrape filter uses — any
//! requested token, as a substring — would have "San Francisco" agreeing with a
//! "San Diego" posting on the shared `san`. Erring toward agreement rather than
//! accusation makes that cheap, not correct: a four-state contract whose one
//! confident state is loose has moved the imprecision rather than removed it.
//!
//! The cost is deliberate and one-directional: a request MORE specific than the
//! posting ("Berlin, Germany" against a bare "Berlin") reads `unknown` rather
//! than `met`. That loses a true positive; it never states a false one, and
//! `unknown` claims nothing.
//!
//! [`ConstraintStatus::NotMet`] therefore has no producer today. It stays in the
//! contract because it is the slot a constraint with real two-sided evidence
//! will use, and because the renderer's `isKnockOut` predicate needs it to
//! exist; `no_shipped_constraint_can_emit_a_knock_out` pins that it is unreachable
//! rather than merely unused.
//!
//! ## Language
//!
//! `languages_align` (mandatory on résumé↔posting SCORING surfaces) does not
//! apply here: nothing in this module reads the résumé or the JD body, stems a
//! token, or extracts a keyword. It compares two short place-name strings, and
//! the language problem that actually bites there — "München" vs "Munich" vs
//! "Muenchen" — is owned by [`location_verdict`]'s exonym table and diaeresis
//! folding, which this module reuses rather than re-deriving.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::scraping::engine::location_filter::{location_verdict, LocationVerdict};
use crate::scraping::types::LocationSpec;

pub(crate) use check::{ConstraintCheck, ConstraintStatus};

/// Stable id of the shipped constraint, on the wire and in the tests.
///
/// `preferredLocation`, not `location`, and the precision is the point: what
/// this check compares is the user's stored **Preferred Location setting**
/// against the posting's location text. It is not a statement about where they
/// live or where they can work. The id is what a renderer keys its i18n off, so
/// naming it this way is what stops the localized sentence from being written as
/// a claim the data cannot support — and it leaves `location` free for a real
/// mobility constraint if candidate-side data for one ever exists.
const PREFERRED_LOCATION: &str = "preferredLocation";

/// Byte cap on each piece of evidence echoed into the payload. The posting's
/// location text is scraped and `job_preferences.location` is renderer-supplied
/// and uncapped at its write boundary, so both are clamped rather than letting an
/// absurd string ride into every match result.
const MAX_EVIDENCE_BYTES: usize = 200;

/// `Some(trimmed)` for a string with content, `None` for absent-or-blank. One
/// helper so "the user left it empty" and "the user never set it" cannot be
/// treated as different kinds of nothing.
fn stated(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// The check type and its ONE constructor, sealed in their own module.
///
/// The nesting is the point. Rust field privacy is module-scoped, so a
/// `ConstraintCheck { .. }` struct literal compiles anywhere inside
/// `mod constraints` — which is precisely where the next constraint gets
/// written, and precisely the bypass the constructor exists to prevent. One
/// module boundary makes [`ConstraintCheck::new`] the only reachable way to
/// build one, so the guarantee below is structural rather than a comment.
mod check {
    use serde::Serialize;

    use super::stated;

    /// The verdict for ONE hard constraint. Four values, because "we cannot
    /// tell" is not one state and is never a pass.
    ///
    /// The two knowable answers ([`Self::Met`] / [`Self::NotMet`]) are only ever
    /// reachable with positive evidence on both sides. The two unknowable ones
    /// are kept apart because they are differently actionable: the user can fix
    /// [`Self::NoPreference`] by filling in a setting; nothing they do fixes a
    /// posting that simply does not say ([`Self::Unknown`]).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) enum ConstraintStatus {
        /// The posting states this, the candidate has stated their side, and
        /// they agree.
        Met,
        /// The posting states this, the candidate has stated their side, and
        /// they conflict. The only knock-out — and still only ever reported,
        /// never acted on. No shipped constraint produces it today; see the
        /// module doc.
        NotMet,
        /// No verdict was reachable: the posting says nothing about it, or what
        /// is known cannot settle the question. NOT a pass and NOT a fail.
        Unknown,
        /// The candidate has expressed nothing about this constraint, so it
        /// cannot be evaluated at all. Said out loud rather than assumed.
        NoPreference,
    }

    /// One constraint's verdict plus the evidence it rests on.
    ///
    /// The two evidence fields are the point, not decoration: carrying each
    /// side's own words makes "positive evidence on BOTH sides" a property of
    /// the payload that a test can check, and lets the renderer compose a
    /// localized sentence instead of shipping English prose from the backend.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ConstraintCheck {
        /// Stable machine id (`"preferredLocation"`), not a label — the
        /// renderer localizes. Named for what is actually compared; see the
        /// constant's doc.
        id: &'static str,
        status: ConstraintStatus,
        /// What the POSTING states about this constraint, verbatim. `None` when
        /// it states nothing.
        #[serde(skip_serializing_if = "Option::is_none")]
        posting: Option<String>,
        /// What the CANDIDATE has stored about it, verbatim. `None` when they
        /// have expressed no preference. For `location` this is a stated
        /// PREFERENCE, not an address and not a mobility limit — see the module
        /// doc.
        #[serde(skip_serializing_if = "Option::is_none")]
        candidate: Option<String>,
    }

    impl ConstraintCheck {
        /// The only way to build a check.
        ///
        /// Rule 3 is enforced **here, at construction**, rather than as a pass
        /// over the finished list: a [`ConstraintStatus::NotMet`] that is not
        /// backed by evidence from BOTH sides is downgraded to
        /// [`ConstraintStatus::Unknown`]. A post-pass would be one `evaluate`
        /// edit away from being skipped, and would leave the guard testable only
        /// in isolation from the checks it guards.
        ///
        /// This is a floor, not the whole rule: two non-empty strings satisfy it
        /// while still not being evidence of a CONFLICT (a failed substring
        /// match is not a contradiction). Deciding that is each check's own job
        /// — see `location_check`, which is why nothing emits `NotMet` today.
        pub(crate) fn new(
            id: &'static str,
            status: ConstraintStatus,
            posting: Option<String>,
            candidate: Option<String>,
        ) -> Self {
            let two_sided =
                stated(posting.as_deref()).is_some() && stated(candidate.as_deref()).is_some();
            let status = if status == ConstraintStatus::NotMet && !two_sided {
                ConstraintStatus::Unknown
            } else {
                status
            };
            Self {
                id,
                status,
                posting,
                candidate,
            }
        }

        /// Read accessors are `#[cfg(test)]` because production genuinely has no
        /// reader: this type exists to be SERIALIZED, and serde's derive reads
        /// the private fields directly. Gating them on test says that out loud
        /// (and keeps `dead_code` honest) instead of silencing the warning with
        /// an `allow`.
        #[cfg(test)]
        pub(crate) fn id(&self) -> &'static str {
            self.id
        }

        #[cfg(test)]
        pub(crate) fn status(&self) -> ConstraintStatus {
            self.status
        }

        #[cfg(test)]
        pub(crate) fn posting(&self) -> Option<&str> {
            self.posting.as_deref()
        }

        #[cfg(test)]
        pub(crate) fn candidate(&self) -> Option<&str> {
            self.candidate.as_deref()
        }
    }
}

/// The posting-side facts a constraint pass reads. Deliberately NOT the JD body:
/// these are the posting's own structured claims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PostingFacts {
    /// The posting's location text as the board gave it.
    pub location: Option<String>,
    /// The board's own `remote` flag. Written by exactly three remote-only
    /// boards (Remotive/RemoteOK/WWR); the aggregator that supplies most
    /// postings never sets it, so `false` here means "not asserted", never "not
    /// remote". Flattened to the top level of the cached posting JSON by
    /// `JobPosting`'s `#[serde(flatten)] extra`.
    pub board_remote: bool,
}

/// The candidate-side facts a constraint pass reads — everything the app
/// actually persists that bears on a non-negotiable. Short on purpose; see the
/// module doc for what is missing and why nothing was invented to fill it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CandidateFacts {
    /// `job_preferences.location` — the free-text "Preferred Location" setting.
    ///
    /// Deliberately the only field here. `job_preferences.country_code` is NOT
    /// read: no renderer surface ever writes it (both call sites only read it,
    /// to seed a search), and `location_filter`'s matcher draws needles from
    /// city/region text only, so a country code contributes nothing to the
    /// comparison. Threading a field that is inert on both ends would be a claim
    /// the code does not honour.
    pub location: Option<String>,
}

/// Evidence as it goes on the wire: trimmed and byte-capped.
fn evidence(s: &str) -> String {
    crate::applications::clamp_to_bytes(s.trim().to_string(), MAX_EVIDENCE_BYTES)
}

/// The location constraint — **`Met` / `Unknown` / `NoPreference` only.**
///
/// The comparison is [`location_verdict`], the same matcher the scrape-time
/// location post-filter runs, so the remote-marker list, the diaeresis folding
/// and the curated exonym table have exactly one home. What differs is the
/// epistemics, and deliberately so: that matcher answers "should this row be
/// dropped from THIS search?", which is a cheap, reversible, conservative call
/// against a location the user typed seconds ago. This function answers "should
/// the user be told not to bother?", against a settings field and a posting that
/// may have arrived from an entirely different search.
///
/// Both directions are re-derived, not just the negative one:
///
/// - `Mismatch` — only ever the ABSENCE of a substring hit — maps to
///   [`ConstraintStatus::Unknown`], not to a knock-out. See the module doc for
///   the concrete pairs (`Germany`/`Berlin`, `Vienna`/`Wien`, `Berlin`/`EMEA`)
///   that make it absence of evidence rather than evidence of conflict.
/// - `PlaceIncomplete` maps to `Unknown` too. `Met` is a positive claim
///   rendered to the user, so it takes the strict whole-token reading: a
///   four-state contract whose one confident state is loose has moved the
///   imprecision rather than removed it. The looser reading stays where it
///   belongs — in the scrape filter, which keeps the row.
///
/// What `Met` claims is bounded to match what is knowable: the stored PREFERENCE
/// matches this posting. Not that the user can work there, and not that it
/// matches the search they are running — see the module doc for why that
/// distinction is what makes `Met` survive the facts that killed `NotMet`.
fn location_check(posting: &PostingFacts, candidate: &CandidateFacts) -> ConstraintCheck {
    // Clamp BEFORE comparing, on both sides, so the verdict and the evidence are
    // derived from the same bytes. Deriving them from different bytes lets a
    // >200-byte location match on a token that then falls outside the reported
    // evidence — a `met` sitting beside a string that no longer contains the
    // place that produced it, which is precisely the two-sided-evidence property
    // the sealed constructor exists to keep checkable.
    let posting_evidence = stated(posting.location.as_deref()).map(evidence);
    // No stored preference → nothing to compare against. Not a pass.
    let Some(candidate_location) = stated(candidate.location.as_deref()).map(evidence) else {
        return ConstraintCheck::new(
            PREFERRED_LOCATION,
            ConstraintStatus::NoPreference,
            posting_evidence,
            None,
        );
    };
    // `region`/`country_code` stay empty: `job_preferences` has no region, and a
    // country code yields no matchable token (see `CandidateFacts::location`).
    let requested = LocationSpec {
        city: Some(candidate_location.clone()),
        ..Default::default()
    };
    let status = match location_verdict(
        posting_evidence.as_deref(),
        posting.board_remote,
        &requested,
    ) {
        // The posting says it is remote, so where the user is looking cannot
        // conflict with it. Positive evidence, no place comparison needed.
        LocationVerdict::Remote => ConstraintStatus::Met,
        // Every place token the user typed is present as a WHOLE token of the
        // posting's location. The only place-based claim strong enough to state.
        LocationVerdict::PlaceMatch => ConstraintStatus::Met,
        // Everything else is "we could not establish it", in one direction or
        // the other, and all three publish as Unknown:
        //
        // - `PlaceIncomplete` — something matched, but the request is not
        //   fully accounted for ("San Francisco" against a "San Diego" posting,
        //   on the shared `san`; or "Berlin, Germany" against a bare "Berlin").
        //   Keeping that row in a search is the right conservative call;
        //   claiming the stored preference matches would overstate it.
        // - `Mismatch` — a failed substring search, which is the ABSENCE of a
        //   hit, not a contradiction. `Germany`/`Berlin`, `Vienna`/`Wien`,
        //   `Berlin`/`EMEA` all land here.
        // - `Undecided` — nothing comparable on one side or the other.
        LocationVerdict::PlaceIncomplete
        | LocationVerdict::Mismatch
        | LocationVerdict::Undecided => ConstraintStatus::Unknown,
    };
    ConstraintCheck::new(
        PREFERRED_LOCATION,
        status,
        posting_evidence,
        Some(candidate_location),
    )
}

/// Evaluate every shipped hard constraint. Pure — the whole decision surface of
/// this module, with no `AppHandle` in sight.
pub(crate) fn evaluate(posting: &PostingFacts, candidate: &CandidateFacts) -> Vec<ConstraintCheck> {
    vec![location_check(posting, candidate)]
}

/// Read the posting-side facts out of one cached posting JSON value. Pure.
///
/// `remote` is read from the TOP level (not under `extra`) because
/// `JobPosting::extra` is `#[serde(flatten)]`.
///
/// `pub(super)` so [`super::match_resume`] calls this inside the ONE
/// `PostingsCache` lock it already takes to resolve the JD text, instead of this
/// module taking the lock a second time and re-running the same linear scan. The
/// duplicate mattered because the verdict is deliberately recomputed on a
/// `match_scores` cache HIT — the Jobs-page path where the score itself costs
/// nothing — so the second scan would have run on literally every call.
pub(super) fn posting_facts_from_value(posting: &Value) -> PostingFacts {
    PostingFacts {
        location: posting
            .get("location")
            .and_then(Value::as_str)
            .map(str::to_string),
        board_remote: posting
            .get("remote")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// The candidate side, read from app state. A `try_state` — the job-preferences
/// store's `open` is non-fatal at setup, and a missing store must read as "no
/// preference expressed", never as a pass. A single-row `SELECT`, not a scan.
///
/// The posting side is NOT read here: the caller already holds it (see
/// [`posting_facts_from_value`]).
fn candidate_facts(app: &AppHandle) -> CandidateFacts {
    app.try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|store| CandidateFacts {
            location: store.get().location,
        })
        .unwrap_or_default()
}

/// Merge a constraint report into a `MatchScore` value as its own `constraints`
/// field, leaving every scoring field byte-identical.
///
/// An `{ "error": … }` value (job not in cache) is returned untouched, and
/// `checks` is not even evaluated: there is no posting to state anything, so
/// there is nothing to report about it. Takes a closure rather than a `Vec` so
/// that skip is real work avoided, and so the whole merge is testable without an
/// `AppHandle`.
fn merge(mut score: Value, checks: impl FnOnce() -> Vec<ConstraintCheck>) -> Value {
    if score.get("error").is_some() {
        return score;
    }
    let Some(obj) = score.as_object_mut() else {
        return score;
    };
    obj.insert("constraints".to_string(), json!({ "checks": checks() }));
    score
}

/// Evaluate the hard constraints and merge the report into `score`. The command
/// layer's single entry point — see [`merge`] for the shape.
///
/// `posting` is resolved by the caller, which already has the cached posting in
/// hand under its own lock. The only state this reaches for is the candidate
/// side, and only when there is something to report about.
pub(crate) fn attach(app: &AppHandle, posting: &PostingFacts, score: Value) -> Value {
    merge(score, || evaluate(posting, &candidate_facts(app)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn posting(location: Option<&str>, board_remote: bool) -> PostingFacts {
        PostingFacts {
            location: location.map(str::to_string),
            board_remote,
        }
    }

    fn candidate(location: Option<&str>) -> CandidateFacts {
        CandidateFacts {
            location: location.map(str::to_string),
        }
    }

    /// The one shipped check, by id.
    fn only(checks: &[ConstraintCheck]) -> &ConstraintCheck {
        assert_eq!(checks.len(), 1, "exactly one constraint ships today");
        assert_eq!(checks[0].id(), "preferredLocation");
        &checks[0]
    }

    /// Status for one (preference, posting location, board remote flag) triple.
    fn status_of(pref: &str, posting_location: &str, board_remote: bool) -> ConstraintStatus {
        let checks = evaluate(
            &posting(Some(posting_location), board_remote),
            &candidate(Some(pref)),
        );
        only(&checks).status()
    }

    // ── location: the positive case ───────────────────────────────────────────

    #[test]
    fn location_is_met_when_the_posting_names_the_candidates_stored_place() {
        let checks = evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some("Berlin")),
        );
        let check = only(&checks);
        assert_eq!(check.status(), ConstraintStatus::Met);
        // Both sides' own words are carried, verbatim.
        assert_eq!(check.posting(), Some("Berlin, Germany"));
        assert_eq!(check.candidate(), Some("Berlin"));
    }

    #[test]
    fn a_remote_posting_is_met_against_any_stored_place() {
        // By the board flag, even with a far-away location text …
        assert_eq!(
            status_of("Berlin", "Austin, TX", true),
            ConstraintStatus::Met
        );
        // … and by a marker in the location text alone.
        assert_eq!(
            status_of("Berlin", "Remote — Worldwide", false),
            ConstraintStatus::Met
        );
    }

    #[test]
    fn location_is_met_across_a_curated_exonym_pair() {
        // Proof the shared matcher is what decides: "Munich" ↔ "München" is
        // bridged only by `location_filter`'s curated table, which a forked
        // matcher here would not have.
        assert_eq!(
            status_of("Munich", "München, Bayern", false),
            ConstraintStatus::Met
        );
    }

    // ── location: everything else is Unknown, never a knock-out ───────────────

    /// The correction this module was rewritten for.
    ///
    /// Every pair below is a job the candidate could plausibly take, and every
    /// one of them made the FIRST cut of this module publish `notMet` — the
    /// strongest negative signal the payload has. A failed substring search is
    /// absence of evidence, not evidence of conflict, so all of them are
    /// `Unknown` now.
    #[test]
    fn a_failed_place_name_match_is_unknown_not_a_knock_out() {
        // (stored preference, posting location, why the substring search fails)
        let takeable: &[(&str, &str, &str)] = &[
            ("Germany", "Berlin", "granularity: country vs city"),
            ("Berlin", "Germany", "granularity: city vs country"),
            ("Vienna", "Wien", "exonym outside the curated table"),
            ("NYC", "New York, NY", "abbreviation"),
            ("San Francisco", "Bay Area", "metro synonym"),
            ("Tokyo", "東京", "non-Latin script"),
            (
                "Berlin",
                "Telecommute",
                "remote synonym off the marker list",
            ),
            ("Berlin", "Virtual", "remote synonym off the marker list"),
            ("Berlin", "Multiple locations", "not a place name"),
            ("Berlin", "EMEA", "region, not a place name"),
            (
                "Berlin",
                "HQ: Austin, TX",
                "unknowable from a settings field",
            ),
        ];
        for (pref, loc, why) in takeable {
            let got = status_of(pref, loc, false);
            assert_eq!(
                got,
                ConstraintStatus::Unknown,
                "{pref:?} vs {loc:?} ({why}) must be unknown, never a knock-out"
            );
        }
        // The three the review named explicitly, asserted individually so a
        // corpus edit cannot quietly drop them.
        assert_eq!(
            status_of("Germany", "Berlin", false),
            ConstraintStatus::Unknown
        );
        assert_eq!(
            status_of("Vienna", "Wien", false),
            ConstraintStatus::Unknown
        );
        assert_eq!(
            status_of("Berlin", "Multiple locations", false),
            ConstraintStatus::Unknown
        );
    }

    /// A partial place-name overlap is not agreement.
    ///
    /// The scrape filter deliberately keeps a "San Diego" row for a "San
    /// Francisco" search — discarding a job is the expensive mistake there. This
    /// pass must not turn that same conservatism into a claim, because here the
    /// expensive mistake is telling the user something false. Same matcher,
    /// opposite risk, so the epistemics differ.
    #[test]
    fn a_partial_place_name_overlap_is_unknown_not_met() {
        // (preference, posting location, what is only partially shared)
        let partial: &[(&str, &str, &str)] = &[
            ("San Francisco", "San Diego, CA", "the `san` token only"),
            ("San Francisco", "Santa Monica, CA", "`san` inside `santa`"),
            (
                "Berlin, Germany",
                "Berlin",
                "a requested token with no home",
            ),
            ("New York", "Newark, NJ", "`new` inside `newark`"),
        ];
        for (pref, loc, why) in partial {
            assert_eq!(
                status_of(pref, loc, false),
                ConstraintStatus::Unknown,
                "{pref:?} vs {loc:?} ({why}) must not read as a match"
            );
        }
        // The control: the SAME preference against a posting that really does
        // name it reads met, so this is a strictness rule and not a blanket
        // refusal to ever match a two-token place.
        assert_eq!(
            status_of("San Francisco", "San Francisco, CA", false),
            ConstraintStatus::Met
        );
    }

    #[test]
    fn location_is_unknown_when_the_posting_states_no_location() {
        for absent in [None, Some(""), Some("   ")] {
            let checks = evaluate(&posting(absent, false), &candidate(Some("Berlin")));
            let check = only(&checks);
            assert_eq!(
                check.status(),
                ConstraintStatus::Unknown,
                "posting location {absent:?} must be unknown, never a pass or a fail"
            );
            assert_eq!(check.posting(), None);
            assert_eq!(check.candidate(), Some("Berlin"));
        }
    }

    #[test]
    fn location_is_no_preference_when_the_candidate_stored_nothing() {
        for empty in [None, Some(""), Some("  ")] {
            let checks = evaluate(&posting(Some("Austin, TX"), false), &candidate(empty));
            let check = only(&checks);
            // Distinct from Unknown: the user can fix this one.
            assert_eq!(check.status(), ConstraintStatus::NoPreference);
            assert_ne!(check.status(), ConstraintStatus::Unknown);
            assert_eq!(check.candidate(), None);
            // The posting's evidence is still reported.
            assert_eq!(check.posting(), Some("Austin, TX"));
        }
    }

    /// The wire id names what is actually compared.
    ///
    /// `preferredLocation`, not `location`: this check reads the user's stored
    /// Preferred Location SETTING, and the id is what a renderer keys its i18n
    /// off. Renaming it to `location` would license "you can't work there" /
    /// "this posting matches where you're looking" — sentences the data does not
    /// support — so the id is pinned rather than left to drift.
    #[test]
    fn the_check_is_named_for_the_setting_it_reads() {
        let checks = evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some("Berlin")),
        );
        assert_eq!(checks[0].id(), "preferredLocation");
        assert_ne!(
            checks[0].id(),
            "location",
            "`location` would read as a claim about where the user can work"
        );
    }

    /// The shipped UI location defaults must produce `met`.
    ///
    /// Every one of them ends in a two-letter qualifier, and every one reads as
    /// a match ONLY because `MIN_TOKEN_LEN` (3) drops that qualifier before
    /// comparison. That constant's own doc justifies 3 as scrape-filter noise
    /// control and says nothing about a published verdict depending on it, so
    /// lowering it to 2 for a scrape-side reason would flip all three of these
    /// to `unknown` with nothing else failing. This is the test that fails.
    #[test]
    fn the_shipped_ui_location_defaults_read_as_a_match() {
        // (JobLocationPreferences' COMMON_LOCATIONS entry, a realistic posting)
        let defaults: &[(&str, &str)] = &[
            ("San Francisco, CA", "San Francisco, California"),
            ("New York, NY", "New York, United States"),
            ("London, UK", "London, United Kingdom"),
            // No two-letter qualifier, included so the case is covered either way.
            ("Berlin, Germany", "Berlin, Germany"),
        ];
        for (pref, posting_location) in defaults {
            assert_eq!(
                status_of(pref, posting_location, false),
                ConstraintStatus::Met,
                "the shipped default {pref:?} must still match {posting_location:?}"
            );
        }
    }

    /// The verdict and the evidence are derived from the SAME bytes.
    ///
    /// Re-running the evaluation on exactly what the check REPORTS must
    /// reproduce exactly what it decided. When the two came from different bytes
    /// — an unclamped location compared, a clamped one reported — a location
    /// longer than the cap could match on a token that then fell outside the
    /// evidence, leaving `met` beside a string not containing the place that
    /// produced it. The board flag is passed through since it is not evidence
    /// text.
    #[test]
    fn a_verdict_is_reproducible_from_the_evidence_it_reports() {
        let long_tail = format!("{} Berlin", "x".repeat(300));
        let long_head = format!("Berlin {}", "x".repeat(300));
        let places = [
            None,
            Some("Berlin, Germany"),
            Some("Austin, TX"),
            Some("Remote"),
            Some(long_tail.as_str()),
            Some(long_head.as_str()),
        ];
        // A preference LONGER than the cap belongs here too: the candidate side
        // is clamped for the payload exactly like the posting side, so comparing
        // the unclamped preference would break the same property in the mirror
        // direction. This one puts the meaningful token past the cut.
        let long_pref_tail = format!("{} Berlin", "x".repeat(300));
        let prefs = [
            None,
            Some("Berlin"),
            Some("San Francisco"),
            Some(long_pref_tail.as_str()),
        ];
        let mut checked = 0;
        for p in places {
            for c in prefs {
                for remote in [false, true] {
                    let facts = posting(p, remote);
                    let cand = candidate(c);
                    let check = &evaluate(&facts, &cand)[0];
                    // Rebuild both sides from what was REPORTED and re-decide.
                    let replayed = &evaluate(
                        &PostingFacts {
                            location: check.posting().map(str::to_string),
                            board_remote: facts.board_remote,
                        },
                        &CandidateFacts {
                            location: check.candidate().map(str::to_string),
                        },
                    )[0];
                    assert_eq!(
                        check.status(),
                        replayed.status(),
                        "status must be reproducible from the reported evidence: {check:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 48); // 6 places × 4 preferences × 2 flags
                                 // The case that used to disagree, pinned concretely: the matching token
                                 // sits past the byte cap, so it is NOT in the evidence and must NOT
                                 // produce a match.
        let past_the_cap = &evaluate(
            &posting(Some(&long_tail), false),
            &candidate(Some("Berlin")),
        )[0];
        assert_eq!(past_the_cap.status(), ConstraintStatus::Unknown);
        assert!(!past_the_cap.posting().unwrap().contains("Berlin"));
        // …while the same token INSIDE the cap still matches.
        let inside_the_cap = &evaluate(
            &posting(Some(&long_head), false),
            &candidate(Some("Berlin")),
        )[0];
        assert_eq!(inside_the_cap.status(), ConstraintStatus::Met);
        assert!(inside_the_cap.posting().unwrap().contains("Berlin"));
        // The mirror: an over-long PREFERENCE is clamped before comparison too,
        // so a token of it past the cut cannot produce a match it does not
        // report. Without this the candidate-side half of the fix is untested.
        let long_pref = &evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some(&long_pref_tail)),
        )[0];
        assert_eq!(long_pref.status(), ConstraintStatus::Unknown);
        assert!(!long_pref.candidate().unwrap().contains("Berlin"));
    }

    // ── the invariant that replaces the old knock-out count ───────────────────

    /// No input reaches `NotMet` through the shipped checks.
    ///
    /// The inverse of the assertion this test replaces. The old one counted the
    /// knock-outs a hand-picked corpus produced and asserted the count — which
    /// could only ever confirm the code agreed with itself, because the corpus
    /// held only pairings it already agreed with. This asserts an absolute zero
    /// over a corpus deliberately stocked with the pairs that used to fail, and
    /// anchors the non-vacuity from both ends: every cell yields exactly one
    /// check, and a known number of them are `Met`.
    #[test]
    fn no_shipped_constraint_can_emit_a_knock_out() {
        let places = [
            None,
            Some(""),
            Some("Berlin, Germany"),
            Some("Austin, TX"),
            Some("Remote"),
            Some("München"),
            Some("Wien"),
            Some("Multiple locations"),
            Some("EMEA"),
            Some("東京"),
            // Carries the partial-overlap pairing (with the "San Francisco"
            // preference below) so the strict-Met rule is exercised by the
            // distribution, not only by the dedicated test.
            Some("San Diego, CA"),
        ];
        let prefs = [
            None,
            Some(""),
            Some("Berlin"),
            Some("DE"),
            Some("Munich"),
            Some("Vienna"),
            Some("Germany"),
            Some("San Francisco"),
        ];
        let (mut total, mut met, mut unknown, mut no_pref) = (0, 0, 0, 0);
        for p in places {
            for c in prefs {
                for remote in [false, true] {
                    let checks = evaluate(&posting(p, remote), &candidate(c));
                    assert_eq!(checks.len(), 1, "each cell yields exactly one check");
                    for check in checks {
                        total += 1;
                        match check.status() {
                            ConstraintStatus::NotMet => {
                                panic!("no shipped constraint may accuse: {check:?}")
                            }
                            ConstraintStatus::Met => met += 1,
                            ConstraintStatus::Unknown => unknown += 1,
                            ConstraintStatus::NoPreference => no_pref += 1,
                        }
                    }
                }
            }
        }
        // The whole distribution, hand-derived, so this cannot pass by the
        // corpus quietly collapsing to one answer. 11 places × 8 preferences
        // × 2 remote flags.
        assert_eq!(total, 176);
        // 2 blank preferences × 11 places × 2 flags — checked before anything
        // about the posting is read.
        assert_eq!(no_pref, 44);
        // 66 from board_remote=true with a stored preference (6 × 11), plus 9
        // at remote=false: "Berlin, Germany" for both Berlin and Germany,
        // "München" for Munich via the exonym table, and "Remote" for all 6
        // non-blank preferences via the marker list.
        //
        // NOT among them: "San Francisco" against "San Diego, CA". That pair
        // shares the `san` token and was `met` under the loose reading — it is
        // the cell that fails this assertion if the strict whole-token rule is
        // ever relaxed.
        assert_eq!(met, 66 + 9);
        // Everything left over — including every pair in the table above.
        assert_eq!(unknown, 57);
        assert_eq!(met + unknown + no_pref, total);
    }

    /// The constructor refuses to build a one-sided accusation. Driven through
    /// `ConstraintCheck::new` — the only reachable way any check is ever built,
    /// now that the type is sealed in its own module — so this cannot pass while
    /// the production path bypasses the guard.
    #[test]
    fn an_unevidenced_accusation_is_downgraded_to_unknown_at_construction() {
        let some = || Some("Austin, TX".to_string());
        let one_sided = [
            (None, Some("Berlin".to_string())),         // posting silent
            (some(), None),                             // candidate silent
            (Some("  ".to_string()), Some("B".into())), // blank is not evidence
            (some(), Some("   ".to_string())),
        ];
        for (posting, candidate) in one_sided {
            let check = ConstraintCheck::new(
                PREFERRED_LOCATION,
                ConstraintStatus::NotMet,
                posting,
                candidate,
            );
            assert_eq!(
                check.status(),
                ConstraintStatus::Unknown,
                "a knock-out without evidence on both sides must not survive construction: {check:?}"
            );
        }
        // A fully-evidenced knock-out is left alone — the floor is two-sided
        // evidence, and judging whether that evidence is a CONFLICT is each
        // check's own job (which is why `location_check` never asks for one).
        let real = ConstraintCheck::new(
            PREFERRED_LOCATION,
            ConstraintStatus::NotMet,
            some(),
            Some("Berlin".to_string()),
        );
        assert_eq!(real.status(), ConstraintStatus::NotMet);
        // The guard only ever touches NotMet: a one-sided Met/Unknown/
        // NoPreference is legitimate and passes through unchanged.
        for status in [
            ConstraintStatus::Met,
            ConstraintStatus::Unknown,
            ConstraintStatus::NoPreference,
        ] {
            let check = ConstraintCheck::new(PREFERRED_LOCATION, status, None, None);
            assert_eq!(check.status(), status);
        }
    }

    // ── payload shape / non-contamination ─────────────────────────────────────

    /// The one line that makes any of this reachable.
    ///
    /// `#[tauri::command] match_resume` needs an `AppHandle`, so no test in this
    /// crate can call it — delete `constraints::attach` from it and every test
    /// above stays green while the feature silently disappears from the app.
    /// (Verified: that mutation survived the whole suite before this test
    /// existed.) A compile-time source scan is the cheapest honest pin, and the
    /// same technique `tests/architecture.rs` already uses for invariants a
    /// linked build cannot see.
    ///
    /// The call is pinned as the function's TAIL EXPRESSION — the exact leading
    /// indentation, then the closing brace on the next line. A weaker
    /// `contains("constraints::attach")` would stay green for
    /// `let _ = constraints::attach(…);` or a commented-out call, both of which
    /// keep the text while dropping the feature; the discard form is rejected
    /// explicitly too, so the failure names the mistake.
    #[test]
    fn the_match_resume_command_returns_the_attached_report() {
        const COMMAND_SRC: &str = include_str!("../match_resume.rs");
        // Matched as a WHOLE LINE, which is what makes the weak forms fail: a
        // commented-out call carries a `//` prefix, and the discard form starts
        // `let _ =`, so neither is equal to this.
        const CALL: &str = "    constraints::attach(&app, &posting_facts, scored)";
        assert!(
            COMMAND_SRC.lines().any(|line| line == CALL),
            "commands::match_resume must RETURN score_one's result through \
             constraints::attach — without that call the hard-constraint pass \
             never reaches the payload, and nothing else fails"
        );
        // Nothing here needs to check that the call is the function's LAST
        // expression: `CALL` carries no trailing semicolon, and a `Value`-typed
        // expression statement that is NOT the tail does not compile. The
        // compiler owns that half; this test owns "the line is still there".
        // (Deliberately no closing-brace literal — the egress inventory's
        // stripper counts braces naively and cannot see into string literals, so
        // an unbalanced one here truncates the region it strips from this file.)
        assert!(
            !COMMAND_SRC.contains("let _ = constraints::attach"),
            "discarding the attached report drops the feature while keeping the call"
        );
    }

    /// The scoring fields must come out the other side byte-identical, and the
    /// verdict must arrive as its own sibling field — never folded in.
    #[test]
    fn attaching_a_report_leaves_every_scoring_field_untouched() {
        let scored = json!({
            "resumeId": "r1",
            "jobId": "j1",
            "ats": 40.0,
            "semantic": 80.0,
            "combined": 64.0,
            "gaps": ["kubernetes"],
            "scoreSource": "combined",
        });
        let checks = evaluate(
            &posting(Some("Austin, TX"), false),
            &candidate(Some("Berlin")),
        );
        let merged = merge(scored.clone(), || checks.clone());
        // The verdict arrived as its own sibling field, not folded in anywhere —
        // and the far-away posting reads `unknown`, not `notMet`.
        assert_eq!(
            merged["constraints"],
            json!({ "checks": [{
                "id": "preferredLocation",
                "status": "unknown",
                "posting": "Austin, TX",
                "candidate": "Berlin",
            }] })
        );
        // The number did not move: 64, the absolute value 0.6 × 80 + 0.4 × 40
        // produces.
        assert_eq!(merged["combined"], json!(64.0));
        assert_eq!(merged["ats"], json!(40.0));
        assert_eq!(merged["semantic"], json!(80.0));
        assert_eq!(merged["scoreSource"], json!("combined"));
        assert_eq!(merged["gaps"], json!(["kubernetes"]));
        // And every pre-existing field is byte-identical: the merge adds one key
        // and rewrites none.
        let mut without_constraints = merged.clone();
        without_constraints
            .as_object_mut()
            .unwrap()
            .remove("constraints");
        assert_eq!(without_constraints, scored);
    }

    #[test]
    fn an_error_result_is_returned_untouched_and_costs_no_evaluation() {
        let err = json!({ "error": "job not found in cache: j1" });
        let mut evaluated = false;
        let out = merge(err.clone(), || {
            evaluated = true;
            Vec::new()
        });
        assert_eq!(out, err);
        assert!(out.get("constraints").is_none());
        assert!(
            !evaluated,
            "an error object has no posting to state anything — don't even look"
        );
    }

    #[test]
    fn the_wire_shape_is_camel_case_with_absent_evidence_omitted() {
        let checks = evaluate(&posting(None, false), &candidate(Some("Berlin")));
        assert_eq!(
            serde_json::to_value(&checks).unwrap(),
            json!([{ "id": "preferredLocation", "status": "unknown", "candidate": "Berlin" }])
        );
        let met = evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some("Berlin")),
        );
        assert_eq!(
            serde_json::to_value(&met).unwrap(),
            json!([{
                "id": "preferredLocation",
                "status": "met",
                "posting": "Berlin, Germany",
                "candidate": "Berlin",
            }])
        );
    }

    #[test]
    fn posting_facts_read_the_flattened_remote_flag_and_location() {
        let facts = posting_facts_from_value(&json!({
            "id": "j1",
            "title": "Engineer",
            "location": "Austin, TX",
            // `JobPosting::extra` is `#[serde(flatten)]`, so board metadata sits
            // at the top level of the cached value, not under `extra`.
            "remote": true,
        }));
        assert_eq!(
            facts,
            PostingFacts {
                location: Some("Austin, TX".to_string()),
                board_remote: true,
            }
        );
        // A posting with neither reads as "states nothing", not as false data.
        assert_eq!(
            posting_facts_from_value(&json!({ "id": "j2" })),
            PostingFacts {
                location: None,
                board_remote: false,
            }
        );
    }

    #[test]
    fn evidence_is_byte_capped_on_a_char_boundary() {
        let long = "ü".repeat(400); // 800 bytes
        let checks = evaluate(&posting(Some(&long), false), &candidate(Some(&long)));
        let check = only(&checks);
        let posted = check.posting().unwrap();
        assert_eq!(posted.len(), MAX_EVIDENCE_BYTES);
        assert_eq!(posted.chars().count(), MAX_EVIDENCE_BYTES / 2);
        assert_eq!(check.candidate().unwrap().len(), MAX_EVIDENCE_BYTES);
    }
}
