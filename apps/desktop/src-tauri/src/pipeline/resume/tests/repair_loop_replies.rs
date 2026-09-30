use super::super::stages::criticals_by_section;
use super::super::stages::sections;
use super::support::{
    live_deadline, repair_report, repair_revalidate, REPAIR_DRAFT, REPAIR_FIXED_SUMMARY,
};
use crate::validate::content::{validate_content, ContentInput, DocKind};

/// Audit finding #2 (HIGH) — a replacement carrying the WHOLE document body
/// (every section, not just the one asked for) used to pass
/// `is_usable_replacement`'s shape checks — it opens with a heading and
/// carries a body line under it — and got spliced in whole, doubling every
/// section it named at export (`model::transform::linearize`'s stable sort
/// merely parks the duplicates adjacent, it does not drop them).
/// `is_usable_replacement` now also rejects a replacement carrying a SECOND
/// detected heading.
///
/// The `regenerate` closure here runs the SAME gate `regenerate_one_section`
/// runs — `sections::accepts`, the real production predicate, not a
/// hand-rebuilt copy of it — against a canned reply, standing in for the
/// provider call the way every other `repair_loop` test in this file does —
/// `regenerate_one_section` itself is a thin `Completer`-calling shim around
/// exactly this gate, and a `Completer` needs a live `AppHandle`.
///
/// Mutation check: drop the `real_section_count(&parsed) <= 1` term from
/// `is_usable_replacement` (which `sections::accepts` calls) and this goes
/// red — the whole draft gets spliced back into itself, `WORK EXPERIENCE`
/// appears twice, and the document changes even though nothing needed to —
/// verified, then restored and re-verified green.
#[tokio::test]
async fn a_whole_document_reply_is_rejected_rather_than_doubling_every_section() {
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |key, document, _issues| {
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the section exists");
            // The model answers with the WHOLE document instead of the one
            // section it was asked for — the exact over-eager reply the
            // audit measured.
            let replacement = document.clone();
            // Finding 5, PR #1003 — premise: `sections::accepts` runs BOTH
            // `is_usable_replacement` (the heading-count check this test
            // exists to exercise) AND `matches_requested_kind` (an identity
            // check). If the whole-document reply's own first heading did NOT
            // match the section under repair, `matches_requested_kind` alone
            // would reject it — and the mutation check above (drop
            // `real_section_count`'s `<= 1` term) would then NOT flip this
            // test red, because the identity mismatch would still reject it
            // on its own. `document.clone()`'s first heading is always
            // REPAIR_DRAFT's first section ("PROFESSIONAL SUMMARY"), so this
            // only holds when the section under repair IS that same kind;
            // pinned here rather than left implicit.
            assert!(
                sections::matches_requested_kind(&replacement, section.kind),
                "premise: the whole-document reply's own first heading must \
                 match the section kind under repair ({:?}), or this test is \
                 exercising `matches_requested_kind` instead of the heading- \
                 count check it exists to prove",
                section.kind
            );
            let outcome = if sections::accepts(&replacement, section.kind) {
                super::super::stages::SectionOutcome::Replaced(sections::splice(
                    &document,
                    section,
                    &replacement,
                ))
            } else {
                super::super::stages::SectionOutcome::Unusable
            };
            async move { Ok(outcome) }
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("re-validation ran");

    assert_eq!(
        document, REPAIR_DRAFT,
        "a whole-document reply must be rejected, not spliced — the draft is unchanged"
    );
    assert_eq!(
        document.matches("WORK EXPERIENCE").count(),
        1,
        "the document must never carry a doubled section"
    );
    assert_eq!(stats.truncated, 1, "the bad reply is a failed attempt");
}

/// Audit finding #3 (HIGH) — `regenerate_one_section` resolved its target by
/// [`SectionKey`] and then spliced back whatever came back WITHOUT ever
/// re-checking what it got: asked for Summary, handed a Skills section, the
/// shape checks alone waved it through — a well-formed heading with a body —
/// and the splice silently swapped the résumé's Summary for a second Skills
/// section, with nothing naming the loss (the NEXT round's
/// `sections::find(&split, Summary)` would return `None`, i.e. a silent,
/// unreported no-op). `sections::matches_requested_kind` re-classifies the
/// reply through the SAME classifier the split used and rejects a mismatch.
///
/// Same closure shape as the finding-#2 test, for the same reason — this one
/// also calls `sections::accepts`, the shared production predicate, so a
/// mutation to EITHER half of the gate it wraps is caught here exactly as it
/// would be caught in `regenerate_one_section` itself.
///
/// Mutation check: drop the `matches_requested_kind` call from
/// `sections::accepts` and this goes red — the Summary section is replaced by
/// "SKILLS\n\nRust · Python · Kafka" and `PROFESSIONAL SUMMARY` disappears
/// from the document — verified, then restored and re-verified green.
#[tokio::test]
async fn a_wrong_kind_reply_is_rejected_rather_than_replacing_the_wrong_section() {
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |key, document, _issues| {
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the section exists");
            // Asked for Summary, the model hands back a Skills section
            // instead — the exact identity mismatch the audit measured.
            let replacement = "SKILLS\n\nRust · Python · Kafka";
            let outcome = if sections::accepts(replacement, section.kind) {
                super::super::stages::SectionOutcome::Replaced(sections::splice(
                    &document,
                    section,
                    replacement,
                ))
            } else {
                super::super::stages::SectionOutcome::Unusable
            };
            async move { Ok(outcome) }
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("re-validation ran");

    assert_eq!(
        document, REPAIR_DRAFT,
        "a wrong-kind reply must be rejected — the draft is unchanged"
    );
    assert!(
        document.contains("PROFESSIONAL SUMMARY"),
        "the résumé must not lose its Summary section entirely; got {document:?}"
    );
    assert_eq!(stats.truncated, 1, "the bad reply is a failed attempt");
}

/// **`normalize` runs on the round's candidate AFTER the section splices and
/// BEFORE `revalidate`.** A closure that visibly mutates the candidate
/// (uppercases it) must be exactly what `revalidate` receives — proving the
/// seam fires, and fires in the right order, independent of
/// `projects::normalize_projects`'s own logic (which has its own tests).
///
/// `revalidate` is a stub here (not the real validator) on purpose: this test
/// is about ORDERING, not about what an uppercased résumé validates as.
///
/// Mutation check: apply `normalize` to `draft` instead of `candidate` (i.e.
/// before the section-splice loop runs) — `calls[0]` would then be the
/// UN-spliced, uppercased original draft and would not contain the spliced-in
/// fixed summary, failing the second assertion.
#[tokio::test]
async fn the_repair_loop_applies_normalize_after_splicing_and_before_revalidate() {
    use std::sync::{Arc, Mutex};

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_for_revalidate = Arc::clone(&seen);

    let (_document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        1, // one round is enough to observe the ordering
        live_deadline(),
        |key, document, _issues| {
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the summary exists");
            let spliced = sections::splice(&document, section, REPAIR_FIXED_SUMMARY);
            async move { Ok(super::super::stages::SectionOutcome::Replaced(spliced)) }
        },
        |candidate: &str| Some(candidate.to_uppercase()),
        move |candidate: String| {
            seen_for_revalidate.lock().unwrap().push(candidate);
            async move {
                Ok((
                    crate::validate::content::ContentReport {
                        ok: true,
                        issues: Vec::new(),
                        metrics: crate::validate::content::ContentMetrics::default(),
                    },
                    None,
                ))
            }
        },
    )
    .await
    .expect("re-validation ran");

    assert_eq!(stats.rounds, 1);
    let calls = seen.lock().unwrap();
    assert_eq!(calls.len(), 1, "one revalidation call");
    assert_eq!(
        calls[0],
        calls[0].to_uppercase(),
        "revalidate must see the candidate AFTER normalize ran"
    );
    assert!(
        calls[0].contains("BUILT THE LEDGER SERVICE"),
        "…and after the splice too — normalize runs on the SPLICED candidate: {}",
        calls[0]
    );
}

/// **A round spends its budget on the WORST sections, not the alphabetically
/// first ones.** A `BTreeMap` is ordered by wire key — `education` <
/// `experience:0` < `projects` < `skills` < `summary` — so a document with five
/// failing sections starved `summary` deterministically, every round, forever.
///
/// Mutation check: return the `BTreeMap`'s own order and the assertion below
/// fails on the first entry.
#[test]
fn repair_spends_its_round_on_the_sections_with_the_most_criticals() {
    // Two Criticals in the summary (two unsourced figures), one in skills.
    // Percent/three-digit figures: `metrics_in` ignores bare numbers under
    // three digits with no `%`/`x` unit.
    let source = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nSKILLS\nGo, Rust\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    let generated = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% and grew revenue by 220%.\n\nSKILLS\nGo, Rust, Kubernetes across 370 clusters\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    let report = validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });

    let grouped = criticals_by_section(generated, &report);
    assert!(
        grouped.len() >= 2,
        "fixture must fail in at least two sections, or the ordering is untested; got {:?}",
        grouped.iter().map(|(key, _)| key).collect::<Vec<_>>()
    );
    let counts: Vec<usize> = grouped.iter().map(|(_, issues)| issues.len()).collect();
    assert!(
        counts.windows(2).all(|pair| pair[0] >= pair[1]),
        "sections must be ordered worst-first; got {:?}",
        grouped
            .iter()
            .map(|(key, issues)| (key, issues.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        counts[0] > 1,
        "the worst section must carry more than one Critical, or the order is coincidence"
    );
}
