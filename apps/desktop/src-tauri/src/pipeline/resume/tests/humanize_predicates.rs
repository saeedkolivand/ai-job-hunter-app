use super::super::prompts::{HumanizeTier, HUMANIZE_DOCUMENT_CAP};
use super::super::stages::{
    exceeds_humanize_cap, humanize_is_worse, is_usable_rewrite, round_is_worse,
    should_humanize_letter, voice_count, voice_findings,
};
use super::support::{ok_report, voice_report, ANY_TEXT};
use crate::validate::content::{
    ContentIssue, ContentMetrics, ContentReport, VOICE_AI_TELL_LEXICAL,
};
use crate::validate::Severity;

/// Mutation check: fold the Critical into the count too (drop the `is_voice_issue`
/// filter) and this fails.
#[test]
fn voice_count_counts_only_the_voice_family() {
    let mut report = voice_report(&["robust", "leverage"]);
    report.issues.push(ContentIssue {
        severity: Severity::Critical,
        code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
        section: None,
        message: "an invented figure".to_string(),
        evidence: Some("47%".to_string()),
    });
    assert_eq!(
        voice_count(&report),
        2,
        "the factual Critical must not count"
    );
    assert_eq!(voice_count(&ok_report()), 0);
}

/// **The link-line exclusion is per-LINE, not per-issue.** A flagged phrase
/// that shares a line with a URL must never reach `<humanize_findings>` — the
/// system prompt repeats the ban, but a finding that never reaches the model
/// cannot be touched even by a model that ignores it.
///
/// Mutation check: drop the `on_link_line` filter in `voice_findings` and the
/// second assertion below fails (the link-line finding would survive).
#[test]
fn voice_findings_drops_a_flagged_line_that_also_carries_a_url() {
    let document = "SKILLS\nRobust systems design.\n\nPROJECTS\nLedger CLI — leverage the API at \
                     https://example.com/ledger\n";
    let report = voice_report(&["Robust", "leverage"]);
    let findings = voice_findings(&report, document);
    assert_eq!(findings.len(), 1, "only the non-link-line finding survives");
    assert!(findings[0].contains("Robust"));
    assert!(!findings.iter().any(|f| f.contains("leverage")));
}

#[test]
fn voice_findings_is_empty_when_every_flag_sits_on_a_link_line() {
    let document = "PROJECTS\nLedger CLI — leverage the API at https://example.com/ledger\n";
    let report = voice_report(&["leverage"]);
    assert!(voice_findings(&report, document).is_empty());
}

/// **An evidence-less `voice.*` issue is forwarded, not dropped.**
/// `on_link_line` only fires on `Some(evidence)` — `evidence: None` has no
/// span to check for a shared link line, and there is no link risk without
/// one, so `voice_findings` must still surface the finding (via `issue_line`,
/// which renders it without an "offending text" line when there is no
/// evidence to quote).
///
/// Mutation check: make the evidence filter treat `None` as "on a link line"
/// (e.g. `unwrap_or(true)` instead of `is_some_and`) and this finding vanishes.
#[test]
fn voice_findings_forwards_an_evidence_less_issue_without_an_offending_text_line() {
    let mut report = voice_report(&["robust"]);
    report.issues.push(ContentIssue {
        severity: Severity::Warning,
        code: VOICE_AI_TELL_LEXICAL,
        section: None,
        message: "a document-wide voice concern with no single span".to_string(),
        evidence: None,
    });
    let findings = voice_findings(&report, "SOME DOCUMENT\n");
    assert_eq!(findings.len(), 2, "both issues must be forwarded");
    assert!(findings
        .iter()
        .any(|f| f.contains("a document-wide voice concern")
            && !f.to_lowercase().contains("offending text")));
}

/// **The cap is a strict boundary, exactly at [`HUMANIZE_DOCUMENT_CAP`].**
/// `fenced()` truncates with no marker at this same cap — see
/// `exceeds_humanize_cap`'s own doc — so the char AT the cap must still be
/// safe to send, and one char over must refuse.
///
/// Mutation check: flip `>` to `>=` and a document exactly at the cap starts
/// refusing (safe direction, but wrong) — flip it to a smaller/larger
/// constant and this drifts from `HUMANIZE_DOCUMENT_CAP`.
#[test]
fn exceeds_humanize_cap_is_a_strict_boundary_at_the_document_cap() {
    let at_cap = "A".repeat(HUMANIZE_DOCUMENT_CAP);
    let over_cap = "A".repeat(HUMANIZE_DOCUMENT_CAP + 1);
    assert!(!exceeds_humanize_cap(&at_cap), "exactly at the cap is safe");
    assert!(exceeds_humanize_cap(&over_cap), "one char over must refuse");
}

/// Mutation check: drop the length-ratio floor and a truncated answer that is
/// merely non-empty passes.
#[test]
fn is_usable_rewrite_rejects_empty_and_drastically_truncated_answers() {
    let original = "PROFESSIONAL SUMMARY\nA payments engineer with ten years of experience \
                     building ledger systems for regulated banks.\n";
    assert!(!is_usable_rewrite(original, "", HumanizeTier::Resume));
    assert!(!is_usable_rewrite(original, "   ", HumanizeTier::Resume));
    assert!(
        !is_usable_rewrite(original, "Summary.", HumanizeTier::Resume),
        "far below half the original length"
    );
    assert!(is_usable_rewrite(
        original,
        "PROFESSIONAL SUMMARY\nA payments engineer with a decade of experience building ledger \
         systems for regulated banks.\n",
        HumanizeTier::Resume
    ));
}

#[test]
fn is_usable_rewrite_has_nothing_to_compare_against_an_empty_original() {
    assert!(is_usable_rewrite(
        "",
        "anything non-empty",
        HumanizeTier::Resume
    ));
    assert!(!is_usable_rewrite("", "", HumanizeTier::Resume));
}

/// **The tier split, pinned at one exact ratio.** 60% of the original clears
/// the résumé's 50% floor and fails the letter's 90% one — the same candidate
/// length is usable for one tier and unusable for the other, which is the
/// whole point of tiering `is_usable_rewrite` rather than sharing one floor.
///
/// Mutation check: swap the two tier arms in `is_usable_rewrite` and both
/// assertions flip.
#[test]
fn is_usable_rewrite_tier_split_is_pinned_at_sixty_percent() {
    let original = "A".repeat(100);
    let candidate_60_percent = "B".repeat(60);

    assert!(
        is_usable_rewrite(&original, &candidate_60_percent, HumanizeTier::Resume),
        "60% clears the résumé's generous 50% floor"
    );
    assert!(
        !is_usable_rewrite(&original, &candidate_60_percent, HumanizeTier::Letter),
        "60% fails the letter's strict 90% floor — its only backstop against content loss"
    );
}

/// **Exact boundaries, both tiers.** Pins the floor as INCLUSIVE (`>=`, not
/// `>`) at the one length where it matters — one character below the ratio
/// must fail, exactly at it must pass.
///
/// Mutation check: flip either tier's `>=` to `>` and the "at the floor"
/// assertion for that tier fails.
#[test]
fn is_usable_rewrite_resume_floor_boundary_is_inclusive_at_exactly_fifty_percent() {
    let original = "A".repeat(100);
    assert!(
        is_usable_rewrite(&original, &"B".repeat(50), HumanizeTier::Resume),
        "exactly 50% must clear the inclusive floor"
    );
    assert!(
        !is_usable_rewrite(&original, &"B".repeat(49), HumanizeTier::Resume),
        "one character under 50% must fail"
    );
}

#[test]
fn is_usable_rewrite_letter_floor_boundary_is_inclusive_at_exactly_ninety_percent() {
    let original = "A".repeat(100);
    assert!(
        is_usable_rewrite(&original, &"B".repeat(90), HumanizeTier::Letter),
        "exactly 90% must clear the inclusive floor"
    );
    assert!(
        !is_usable_rewrite(&original, &"B".repeat(89), HumanizeTier::Letter),
        "one character under 90% must fail"
    );
}

/// **BUG-A regression — the humanize fence-tag leak.** A real run exported a
/// résumé with `<humanize_document>` as the candidate's name and
/// `</humanize_document>` as its last line: the model echoed the fence wrapper
/// `humanize_user` sent it in, and the length floor alone let it through (a
/// wrapper only ADDS length). Every candidate wrapped in ANY registered fence
/// tag — not just the one that bit us — must now be rejected, driven off
/// `prompt_fence::known_fence_tags()` rather than a single hardcoded name so a
/// future tag added to the registry is covered here for free.
///
/// Mutation check: drop the `contains_fence_tag` gate from `is_usable_rewrite`
/// and every assertion below flips true — verified, then reverted.
#[test]
fn is_usable_rewrite_rejects_a_candidate_wrapped_in_any_registered_fence_tag() {
    let original = "PROFESSIONAL SUMMARY\nA payments engineer with ten years of experience \
                     building ledger systems for regulated banks.\n";
    for tag in crate::prompt_fence::known_fence_tags() {
        let wrapped = format!(
            "<{tag}>\nPROFESSIONAL SUMMARY\nA payments engineer with a decade of experience \
             building ledger systems for regulated banks.\n</{tag}>"
        );
        assert!(
            !is_usable_rewrite(original, &wrapped, HumanizeTier::Resume),
            "a candidate wrapped in <{tag}> must be rejected, not accepted as content-clean"
        );
    }
}

/// **HIGH-1 regression — the letter arm's gate.** `should_humanize_letter`
/// must refuse whenever a run never asked for a letter, and it must do so
/// even if `letter_body` (which production always feeds `ctx.letter`, never
/// `ctx.letter_text()`'s legacy fallback) happens to be non-empty: the flag
/// and the field-read are two INDEPENDENT guards, and this pins that neither
/// one alone is trusted to carry the rule.
///
/// Mutation check: drop the `include_cover_letter` clause from
/// `should_humanize_letter` and the second assertion (a non-empty body with
/// the flag OFF) flips true.
#[test]
fn should_humanize_letter_refuses_whenever_the_run_never_requested_a_letter() {
    // The real-world shape: `includeCoverLetter: false` means `ctx.letter` is
    // ALWAYS empty (`cover_letter` no-ops), so both conditions fail together.
    assert!(!should_humanize_letter(2, "", false));
    // Defense in depth: even a non-empty body cannot make this run without the
    // flag — the case HIGH-1 exists to close (a legacy validate-only
    // `coverLetterText` must never be silently rewritten and persisted).
    assert!(!should_humanize_letter(2, "a stray letter body", false));
    // Nothing flagged is a no-op regardless of the flag.
    assert!(!should_humanize_letter(0, "a generated letter", true));
    // Requested, but the stage produced nothing (yet) to rewrite.
    assert!(!should_humanize_letter(2, "", true));
    // The one case that actually runs: requested, flagged, and non-empty.
    assert!(should_humanize_letter(2, "a generated letter", true));
}

/// **`humanize_is_worse` widens `round_is_worse` by one clause**: more
/// `voice.*` flags than before is worse even with the SAME or FEWER Criticals
/// — `round_is_worse` alone would call this an improvement, because it never
/// looks at Warnings at all.
///
/// Mutation check: drop the `voice_count` clause and this fails.
#[test]
fn humanize_is_worse_reverts_on_more_voice_flags_even_with_no_new_criticals() {
    let before = voice_report(&["robust"]);
    let after = voice_report(&["robust", "leverage"]);
    assert!(
        !round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "round_is_worse alone cannot see a voice-only regression"
    );
    assert!(humanize_is_worse(&before, ANY_TEXT, &after, ANY_TEXT));
}

#[test]
fn humanize_is_worse_keeps_a_candidate_with_fewer_or_equal_voice_flags() {
    let before = voice_report(&["robust", "leverage"]);
    let fewer = voice_report(&["robust"]);
    let equal = voice_report(&["leverage"]);
    assert!(!humanize_is_worse(&before, ANY_TEXT, &fewer, ANY_TEXT));
    assert!(!humanize_is_worse(&before, ANY_TEXT, &equal, ANY_TEXT));
}

/// The `round_is_worse` half of the rule still applies unchanged: fewer voice
/// flags does not excuse a NEW Critical.
#[test]
fn humanize_is_worse_still_reverts_on_a_new_critical_even_with_fewer_voice_flags() {
    let before = voice_report(&["robust", "leverage"]);
    let mut after = voice_report(&["robust"]);
    after.issues.push(ContentIssue {
        severity: Severity::Critical,
        code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
        section: None,
        message: "an invented figure".to_string(),
        evidence: Some("47%".to_string()),
    });
    after.ok = false;
    assert!(humanize_is_worse(&before, ANY_TEXT, &after, ANY_TEXT));
}

/// **The coverage floor**: a rewrite that quietly deletes exact job-ad terms
/// is worse even with NO new Critical and NO new voice flag —
/// `round_is_worse` and `voice_count` are both blind to keyword coverage.
///
/// Mutation check: drop `coverage_dropped` from `humanize_is_worse` and the
/// first assertion fails.
#[test]
fn humanize_is_worse_reverts_on_a_coverage_drop_even_with_nothing_else_worse() {
    let with_coverage = |coverage: f64| ContentReport {
        ok: true,
        issues: Vec::new(),
        metrics: ContentMetrics {
            keyword_coverage: Some(coverage),
            ..ContentMetrics::default()
        },
    };
    let before = with_coverage(80.0);

    // A drop past `MIN_COVERAGE_DROP_POINTS` (5.0) — nothing else about the
    // report changed.
    let dropped = with_coverage(70.0);
    assert!(humanize_is_worse(&before, ANY_TEXT, &dropped, ANY_TEXT));

    // A drop under the threshold is not worth reverting over.
    let small_drop = with_coverage(77.0);
    assert!(!humanize_is_worse(&before, ANY_TEXT, &small_drop, ANY_TEXT));

    // Exactly `MIN_COVERAGE_DROP_POINTS` (5.0) — `coverage_dropped` compares
    // with `>=`, so the threshold itself counts as a drop, not just anything
    // past it. This is the boundary CodeRabbit flagged: the doc comment used
    // to say "fell more than" the threshold, which would have kept this exact
    // case, not reverted it.
    let exactly_at_threshold = with_coverage(75.0);
    assert!(humanize_is_worse(
        &before,
        ANY_TEXT,
        &exactly_at_threshold,
        ANY_TEXT
    ));

    // An uncomparable posting (`None` coverage) never rejects on coverage
    // alone — `round_is_worse`/`voice_count` still decide, and both are clean
    // here.
    let uncomparable = ok_report();
    assert!(!humanize_is_worse(
        &before,
        ANY_TEXT,
        &uncomparable,
        ANY_TEXT
    ));
}
