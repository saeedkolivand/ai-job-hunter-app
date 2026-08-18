//! Hard-constraint pass: the non-negotiables a relevance score cannot carry.
//!
//! `combined = f(semantic, ats)` measures how much a posting's vocabulary the
//! résumé covers. It is a good answer to "is this role about what I do?" and it
//! is structurally incapable of answering "could I actually take this job?" —
//! so a candidate who fails a non-negotiable still scores well on keyword
//! overlap. This module answers the second question **separately** and reports
//! it as its own payload field.
//!
//! ## Three rules, and why each exists
//!
//! 1. **Never touched the score.** The verdict is computed in the L3 command
//!    ([`super::match_resume`]) AFTER the kernel returns, and merged into the
//!    result value. It is not a multiplier, not a penalty, and not an input to
//!    [`super::score_one`] — which means it also never enters the `match_scores`
//!    cache. That is not just hygiene: the cache key is composed of SCORING
//!    inputs only, so a verdict cached under it would go stale the moment the
//!    user edits their preferences and would then be served for the whole TTL.
//! 2. **Never hides a job.** Nothing here filters, sorts, or gates. A wrong
//!    knock-out tells the user not to bother applying, which is far more costly
//!    than a wrong relevance number, so the only thing a `NotMet` does is
//!    appear next to the score.
//! 3. **Absence of evidence is never evidence.** [`ConstraintStatus`] has FOUR
//!    values, and the two "we don't know" ones are distinct from both a pass and
//!    a fail. [`ConstraintStatus::NotMet`] requires positive evidence on BOTH
//!    sides — the posting states the thing, and the candidate's own stored data
//!    contradicts it — and that requirement is enforced structurally, inside
//!    the one constructor every check must go through
//!    ([`ConstraintCheck::new`]), not merely by convention.
//!
//! ## What is checked, and what is deliberately NOT
//!
//! Shipped:
//!
//! - **`location`** — the candidate's stored job-search location
//!   (`job_preferences.location`, optionally with its geocode-picked
//!   `country_code`) against the posting's own location text and its board
//!   remote flag. Both sides are real, persisted, user-entered data.
//!
//! Refused, because the CANDIDATE side does not exist in this app today. Each
//! of these would have required inventing a settings surface and then guessing
//! at the answer, which is how a check comes to accuse on absence of evidence:
//!
//! - **`workAuthorization`** — ranked first by the audit, and the app stores
//!   nothing about it anywhere: no visa/sponsorship/citizenship/right-to-work
//!   field on `ContactProfile` or `JobPreferences`. The tempting proxy — the
//!   candidate's own address — is exactly the false accusation: living in one
//!   country is not evidence of lacking authorization in another.
//! - **`employmentType`** — no persisted candidate preference exists.
//!   `AutopilotTarget::work_type` is a per-autopilot SEARCH filter (and is a
//!   work arrangement: remote/hybrid/on-site, not full-time/contract), scoped
//!   to one autopilot config; the Jobs-page match has no autopilot in hand.
//! - **`salaryFloor`** — `job_preferences.salary_expectation` is free text
//!   ("80k DOE"), documented as the answer to an application's
//!   salary-expectation question. An expectation is not a stated floor, it
//!   carries no currency and no period, and the posting side is a structured
//!   range on ONE board (Adzuna's `salaryMin`/`salaryMax`/`salaryCurrency`).
//!   Comparing them needs a free-text money parser plus an FX assumption —
//!   two invented facts per verdict.
//!
//! ## Language
//!
//! `languages_align` (mandatory on résumé↔posting SCORING surfaces) does not
//! apply here: nothing in this module reads the résumé or the JD body, stems a
//! token, or extracts a keyword. It compares two short place-name strings, and
//! the language problem that actually bites there — "München" vs "Munich" vs
//! "Muenchen" — is owned by [`location_verdict`]'s exonym table and diaeresis
//! folding, which this module reuses rather than re-deriving.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::applications::clamp_to_bytes;
use crate::scraping::engine::location_filter::{location_verdict, LocationVerdict};
use crate::scraping::types::LocationSpec;

/// Stable id of the location constraint, on the wire and in the tests.
const LOCATION: &str = "location";

/// Byte cap on each piece of evidence echoed into the payload. The posting's
/// location text is scraped and `job_preferences.location` is renderer-supplied
/// and uncapped at its write boundary, so both are clamped here rather than
/// letting an absurd string ride into every match result. Cuts on a UTF-8 char
/// boundary (see [`clamp_to_bytes`]).
const MAX_EVIDENCE_BYTES: usize = 200;

/// The verdict for ONE hard constraint. Four values, because "we cannot tell"
/// is not one state and is never a pass:
///
/// The two knowable answers ([`Self::Met`] / [`Self::NotMet`]) are only ever
/// reachable with positive evidence on both sides. The two unknowable ones are
/// kept apart because they are differently actionable: the user can fix
/// [`Self::NoPreference`] by filling in a setting; nothing they do fixes a
/// posting that simply does not say ([`Self::Unknown`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ConstraintStatus {
    /// The posting states this, the candidate has stated their side, and they
    /// agree.
    Met,
    /// The posting states this, the candidate has stated their side, and they
    /// conflict. The only knock-out — and still only ever reported, never acted
    /// on.
    NotMet,
    /// The candidate stated a preference, but no verdict was reachable: the
    /// posting says nothing about it, or what the candidate stated is not
    /// usable for a comparison. NOT a pass and NOT a fail.
    Unknown,
    /// The candidate has expressed nothing about this constraint, so it cannot
    /// be evaluated at all. Said out loud rather than assumed either way.
    NoPreference,
}

/// One constraint's verdict plus the evidence it rests on.
///
/// The two evidence fields are the point, not decoration: carrying each side's
/// own words makes "positive evidence on BOTH sides" a property of the payload
/// that a test can check, and lets the renderer compose a localized sentence
/// instead of shipping English prose from the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConstraintCheck {
    /// Stable machine id (`"location"`), not a label — the renderer localizes.
    id: &'static str,
    status: ConstraintStatus,
    /// What the POSTING states about this constraint, verbatim. `None` when it
    /// states nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    posting: Option<String>,
    /// What the CANDIDATE has stored about it, verbatim. `None` when they have
    /// expressed no preference.
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate: Option<String>,
}

impl ConstraintCheck {
    /// The ONLY way to build a check — the fields are private precisely so this
    /// is unavoidable.
    ///
    /// Rule 3 is enforced **here, at construction**, rather than as a pass over
    /// the finished list: a [`ConstraintStatus::NotMet`] that is not backed by
    /// evidence from BOTH sides is downgraded to [`ConstraintStatus::Unknown`].
    /// A post-pass would be one `evaluate` edit away from being skipped, and
    /// would leave the guard testable only in isolation from the checks it
    /// guards; a constructor cannot be forgotten by a constraint added later.
    ///
    /// The location check below is already written so this can never fire. That
    /// is the point: a knock-out is the one output of this module that costs the
    /// user something (it tells them not to apply), and this repo has a live,
    /// still-open incident from a check that accused on absence of evidence.
    fn new(
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
}

/// The posting-side facts a constraint pass reads. Deliberately NOT the JD body:
/// these are the posting's own structured claims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PostingFacts {
    /// The posting's location text as the board gave it.
    pub location: Option<String>,
    /// The board's own `remote` flag (Remotive/RemoteOK/WWR and the engine's
    /// location filter set it), flattened to the top level of the cached
    /// posting JSON by `JobPosting`'s `#[serde(flatten)] extra`.
    pub board_remote: bool,
}

/// The candidate-side facts a constraint pass reads — everything the app
/// actually persists that bears on a non-negotiable. Short on purpose; see the
/// module doc for what is missing and why nothing was invented to fill it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CandidateFacts {
    /// `job_preferences.location` — the job-search location the user typed or
    /// picked in Settings.
    pub location: Option<String>,
    /// `job_preferences.country_code` — ISO 3166-1 alpha-2, present only when
    /// the location came from a picked geocode suggestion.
    pub country_code: Option<String>,
}

/// `Some(trimmed)` for a string with content, `None` for absent-or-blank. One
/// helper so "the user left it empty" and "the user never set it" cannot be
/// treated as different kinds of nothing.
fn stated(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// Evidence as it goes on the wire: trimmed and byte-capped.
fn evidence(s: &str) -> String {
    clamp_to_bytes(s.trim().to_string(), MAX_EVIDENCE_BYTES)
}

/// The location constraint: can the candidate take the job where it is?
///
/// The comparison itself is [`location_verdict`] — the same matcher the
/// scrape-time location post-filter runs, so the remote-marker list, the
/// diaeresis folding and the curated exonym table have exactly one home. This
/// function's own job is the epistemics: which of the four statuses that
/// three-valued answer maps to, and what evidence backs it.
fn location_check(posting: &PostingFacts, candidate: &CandidateFacts) -> ConstraintCheck {
    let posting_evidence = stated(posting.location.as_deref()).map(evidence);
    // No stored job-search location → nothing to compare against. Not a pass.
    let Some(candidate_location) = stated(candidate.location.as_deref()) else {
        return ConstraintCheck::new(
            LOCATION,
            ConstraintStatus::NoPreference,
            posting_evidence,
            None,
        );
    };
    // `region` stays empty: `job_preferences` has no region column, and
    // `country_code` alone yields no matchable token — a country-code-only
    // preference lands on `Undecided`, which is the honest answer.
    let requested = LocationSpec {
        city: Some(candidate_location.to_string()),
        country_code: stated(candidate.country_code.as_deref()).map(str::to_string),
        ..Default::default()
    };
    let status = match location_verdict(
        stated(posting.location.as_deref()),
        posting.board_remote,
        &requested,
    ) {
        LocationVerdict::Match => ConstraintStatus::Met,
        LocationVerdict::Mismatch => ConstraintStatus::NotMet,
        LocationVerdict::Undecided => ConstraintStatus::Unknown,
    };
    ConstraintCheck::new(
        LOCATION,
        status,
        posting_evidence,
        Some(evidence(candidate_location)),
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
fn posting_facts_from_value(posting: &Value) -> PostingFacts {
    let str_field = |k: &str| posting.get(k).and_then(Value::as_str).map(str::to_string);
    PostingFacts {
        location: str_field("location"),
        board_remote: posting
            .get("remote")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Resolve both sides from app state and evaluate. Every lookup is a
/// `try_state` — the job-preferences store's `open` is non-fatal at setup, and a
/// missing store must read as "no preference expressed", never as a pass.
fn checks_for_job(app: &AppHandle, job_id: &str) -> Vec<ConstraintCheck> {
    let posting = app
        .try_state::<parking_lot::Mutex<crate::postings::PostingsCache>>()
        .and_then(|cache| {
            let guard = cache.lock();
            guard
                .get_all()
                .iter()
                .find(|p| p.get("id").and_then(Value::as_str) == Some(job_id))
                .map(posting_facts_from_value)
        })
        .unwrap_or_default();
    let candidate = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|store| {
            let prefs = store.get();
            CandidateFacts {
                location: prefs.location,
                country_code: prefs.country_code,
            }
        })
        .unwrap_or_default();
    evaluate(&posting, &candidate)
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

/// Evaluate the hard constraints for `job_id` and merge the report into `score`.
/// The command layer's single entry point — see [`merge`] for the shape.
pub(crate) fn attach(app: &AppHandle, job_id: &str, score: Value) -> Value {
    merge(score, || checks_for_job(app, job_id))
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
            country_code: None,
        }
    }

    /// The one shipped check, by id.
    fn only(checks: &[ConstraintCheck]) -> &ConstraintCheck {
        assert_eq!(checks.len(), 1, "exactly one constraint ships today");
        assert_eq!(checks[0].id, "location");
        &checks[0]
    }

    // ── location: positive ────────────────────────────────────────────────────

    #[test]
    fn location_is_met_when_the_posting_names_the_candidates_stored_city() {
        let checks = evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some("Berlin")),
        );
        let check = only(&checks);
        assert_eq!(check.status, ConstraintStatus::Met);
        // Both sides' own words are carried, verbatim.
        assert_eq!(check.posting.as_deref(), Some("Berlin, Germany"));
        assert_eq!(check.candidate.as_deref(), Some("Berlin"));
    }

    #[test]
    fn a_remote_posting_is_met_against_any_stored_city() {
        // By the board flag, even with a far-away location text …
        let by_flag = evaluate(
            &posting(Some("Austin, TX"), true),
            &candidate(Some("Berlin")),
        );
        assert_eq!(only(&by_flag).status, ConstraintStatus::Met);
        // … and by a marker in the location text alone.
        let by_text = evaluate(
            &posting(Some("Remote — Worldwide"), false),
            &candidate(Some("Berlin")),
        );
        assert_eq!(only(&by_text).status, ConstraintStatus::Met);
    }

    #[test]
    fn location_is_met_across_a_curated_exonym_pair() {
        // Proof the shared matcher is what decides: "Munich" ↔ "München" is
        // bridged only by `location_filter`'s curated table, which a forked
        // matcher here would not have.
        let checks = evaluate(
            &posting(Some("München, Bayern"), false),
            &candidate(Some("Munich")),
        );
        assert_eq!(only(&checks).status, ConstraintStatus::Met);
    }

    // ── location: negative ────────────────────────────────────────────────────

    #[test]
    fn location_is_not_met_when_the_posting_names_a_different_place_and_is_not_remote() {
        let checks = evaluate(
            &posting(Some("Austin, TX, United States"), false),
            &candidate(Some("Berlin")),
        );
        let check = only(&checks);
        assert_eq!(check.status, ConstraintStatus::NotMet);
        assert_eq!(check.posting.as_deref(), Some("Austin, TX, United States"));
        assert_eq!(check.candidate.as_deref(), Some("Berlin"));
    }

    // ── location: unknown / not evaluable ─────────────────────────────────────

    #[test]
    fn location_is_unknown_when_the_posting_states_no_location() {
        for absent in [None, Some(""), Some("   ")] {
            let checks = evaluate(&posting(absent, false), &candidate(Some("Berlin")));
            let check = only(&checks);
            assert_eq!(
                check.status,
                ConstraintStatus::Unknown,
                "posting location {absent:?} must be unknown, never a pass or a fail"
            );
            assert_eq!(check.posting, None);
            assert_eq!(check.candidate.as_deref(), Some("Berlin"));
        }
    }

    #[test]
    fn location_is_unknown_when_the_stored_preference_yields_no_matchable_place() {
        // A country-code-only preference: real, but nothing to match a place
        // name against. Undecided — not a silent pass on the posting's city.
        let checks = evaluate(
            &posting(Some("Austin, TX"), false),
            &CandidateFacts {
                location: Some("DE".to_string()),
                country_code: Some("de".to_string()),
            },
        );
        assert_eq!(only(&checks).status, ConstraintStatus::Unknown);
    }

    #[test]
    fn location_is_no_preference_when_the_candidate_stored_nothing() {
        for empty in [None, Some(""), Some("  ")] {
            let checks = evaluate(&posting(Some("Austin, TX"), false), &candidate(empty));
            let check = only(&checks);
            // Distinct from Unknown: the user can fix this one.
            assert_eq!(check.status, ConstraintStatus::NoPreference);
            assert_ne!(check.status, ConstraintStatus::Unknown);
            assert_eq!(check.candidate, None);
            // The posting's evidence is still reported.
            assert_eq!(check.posting.as_deref(), Some("Austin, TX"));
        }
    }

    // ── the invariant ─────────────────────────────────────────────────────────

    #[test]
    fn a_knock_out_always_carries_evidence_from_both_sides() {
        let places = [
            None,
            Some(""),
            Some("Berlin, Germany"),
            Some("Austin, TX"),
            Some("Remote"),
            Some("München"),
        ];
        let prefs = [None, Some(""), Some("Berlin"), Some("DE"), Some("Munich")];
        let mut knock_outs = 0;
        for p in places {
            for c in prefs {
                for remote in [false, true] {
                    for check in evaluate(&posting(p, remote), &candidate(c)) {
                        if check.status != ConstraintStatus::NotMet {
                            continue;
                        }
                        knock_outs += 1;
                        assert!(
                            stated(check.posting.as_deref()).is_some(),
                            "notMet with no posting evidence: {check:?}"
                        );
                        assert!(
                            stated(check.candidate.as_deref()).is_some(),
                            "notMet with no candidate evidence: {check:?}"
                        );
                    }
                }
            }
        }
        // Anchored to a hand-counted absolute so the loop above cannot go
        // vacuously green: the four non-remote conflicting pairs are
        // Munich↔"Berlin, Germany", Berlin↔"Austin, TX", Munich↔"Austin, TX"
        // and Berlin↔"München". Every other cell is met (Berlin↔Berlin,
        // Munich↔München via the exonym table, anything↔"Remote", every
        // remote=true row) or unknowable (blank posting, blank or 2-letter
        // preference).
        assert_eq!(knock_outs, 4);
    }

    /// The constructor refuses to build a one-sided accusation. Driven through
    /// `ConstraintCheck::new` — the only way any check is ever built — so this
    /// cannot pass while the production path bypasses the guard.
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
            let check =
                ConstraintCheck::new(LOCATION, ConstraintStatus::NotMet, posting, candidate);
            assert_eq!(
                check.status,
                ConstraintStatus::Unknown,
                "a knock-out without evidence on both sides must not survive construction: {check:?}"
            );
        }
        // A fully-evidenced knock-out is left alone …
        let real = ConstraintCheck::new(
            LOCATION,
            ConstraintStatus::NotMet,
            some(),
            Some("Berlin".to_string()),
        );
        assert_eq!(real.status, ConstraintStatus::NotMet);
        // … and the guard only ever touches NotMet: a one-sided Met/Unknown/
        // NoPreference is legitimate and passes through unchanged.
        for status in [
            ConstraintStatus::Met,
            ConstraintStatus::Unknown,
            ConstraintStatus::NoPreference,
        ] {
            let check = ConstraintCheck::new(LOCATION, status, None, None);
            assert_eq!(check.status, status);
        }
    }

    // ── payload shape / non-contamination ─────────────────────────────────────

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
        // The verdict arrived as its own sibling field, not folded in anywhere.
        assert_eq!(
            merged["constraints"],
            json!({ "checks": [{
                "id": "location",
                "status": "notMet",
                "posting": "Austin, TX",
                "candidate": "Berlin",
            }] })
        );
        // A failed constraint did not move the number: 64, the absolute value
        // 0.6 × 80 + 0.4 × 40 produces.
        assert_eq!(only(&checks).status, ConstraintStatus::NotMet);
        assert_eq!(merged["combined"], json!(64.0));
        assert_eq!(merged["ats"], json!(40.0));
        assert_eq!(merged["semantic"], json!(80.0));
        assert_eq!(merged["scoreSource"], json!("combined"));
        assert_eq!(merged["gaps"], json!(["kubernetes"]));
        // And every pre-existing field is byte-identical: the merge adds one
        // key and rewrites none.
        let mut without_constraints = merged.clone();
        without_constraints
            .as_object_mut()
            .unwrap()
            .remove("constraints");
        assert_eq!(without_constraints, scored);
    }

    /// The one line that makes any of this reachable.
    ///
    /// `#[tauri::command] match_resume` needs an `AppHandle`, so no test in this
    /// crate can call it — delete `constraints::attach` from it and every test
    /// above stays green while the feature silently disappears from the app.
    /// (Verified: that mutation survived the whole suite before this test
    /// existed.) A compile-time source scan is the cheapest honest pin, and the
    /// same technique `tests/architecture.rs` already uses for invariants a
    /// linked build cannot see.
    #[test]
    fn the_match_resume_command_actually_attaches_the_report() {
        const COMMAND_SRC: &str = include_str!("../match_resume.rs");
        assert!(
            COMMAND_SRC.contains("constraints::attach(&app, &req.job_id, scored)"),
            "commands::match_resume must return score_one's result through \
             constraints::attach — without that call the hard-constraint pass \
             never reaches the payload, and nothing else fails"
        );
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
            json!([{ "id": "location", "status": "unknown", "candidate": "Berlin" }])
        );
        let met = evaluate(
            &posting(Some("Berlin, Germany"), false),
            &candidate(Some("Berlin")),
        );
        assert_eq!(
            serde_json::to_value(&met).unwrap(),
            json!([{
                "id": "location",
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
        let posted = check.posting.as_deref().unwrap();
        assert_eq!(posted.len(), MAX_EVIDENCE_BYTES);
        assert_eq!(posted.chars().count(), MAX_EVIDENCE_BYTES / 2);
        assert_eq!(
            check.candidate.as_deref().unwrap().len(),
            MAX_EVIDENCE_BYTES
        );
    }
}
