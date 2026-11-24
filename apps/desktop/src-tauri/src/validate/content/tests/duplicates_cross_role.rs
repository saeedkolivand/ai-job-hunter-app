//! A bullet repeated under two different roles is fabrication, not padding.

use super::{support::*, *};

const BULLET: &str =
    "- Ran the Docker workloads on a Kubernetes cluster that answers 12000 requests every second";

#[test]
fn the_same_bullet_under_two_roles_is_one_critical() {
    let globex = "- Took the fleet to AWS, with the whole deployment pipeline written as Terraform";
    let doc = EN_CLEAN.replace(globex, &format!("{globex}\n{BULLET}"));
    assert_ne!(doc, EN_CLEAN, "fixture drifted");
    let report = en_resume(&doc, &en_requirements());
    let hits = fired(&report, DUPLICATE_CROSS_ROLE_BULLET);
    assert_eq!(hits.len(), 1, "{:#?}", report.issues);
    assert_eq!(hits[0].severity, Severity::Critical);
    silent(&report, DUPLICATE_BULLET);
    assert!(!report.ok);
}

#[test]
fn a_near_duplicate_inside_one_role_stays_a_warning() {
    let report = en_resume(EN_DUPLICATES, &en_requirements());
    assert_eq!(fired(&report, DUPLICATE_BULLET).len(), 1);
    silent(&report, DUPLICATE_CROSS_ROLE_BULLET);
}

/// Summary copy first, then role A, then role B: the Summary pairs must not mask the
/// role-to-role copy (#1556 review).
#[test]
fn a_summary_copy_seen_first_does_not_mask_a_cross_role_copy() {
    let globex = "- Took the fleet to AWS, with the whole deployment pipeline written as Terraform";
    let summary = "container platforms behind them.";
    let doc = EN_CLEAN
        .replace(globex, &format!("{globex}\n{BULLET}"))
        .replace(summary, &format!("{summary}\n{BULLET}"));
    assert_eq!(doc.matches(BULLET).count(), 3, "fixture drifted");
    let report = en_resume(&doc, &en_requirements());
    assert_eq!(
        fired(&report, DUPLICATE_CROSS_ROLE_BULLET).len(),
        1,
        "{:#?}",
        report.issues
    );
    assert!(!report.ok);
}
