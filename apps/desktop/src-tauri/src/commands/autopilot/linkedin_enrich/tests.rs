use super::*;

// Each test uses its own URL namespace (a unique prefix) so tests running in
// parallel against the same process-global `ENRICHING` set can never
// interfere with each other's claims — no `#[serial]` needed.

#[test]
fn a_second_pass_cannot_claim_a_url_already_in_flight() {
    let claimed_first = claim_unclaimed(vec![
        "https://www.linkedin.com/jobs/view/coalesce-1".to_string(),
        "https://www.linkedin.com/jobs/view/coalesce-2".to_string(),
    ]);
    assert_eq!(
        claimed_first,
        vec![
            "https://www.linkedin.com/jobs/view/coalesce-1".to_string(),
            "https://www.linkedin.com/jobs/view/coalesce-2".to_string(),
        ],
        "the first pass claims every URL nobody else is enriching"
    );

    // A second, overlapping pass (e.g. a manual re-run racing the first
    // pass's still-in-flight enrichment) must NOT re-claim coalesce-1 — only
    // the genuinely-new coalesce-3 gets through.
    let claimed_second = claim_unclaimed(vec![
        "https://www.linkedin.com/jobs/view/coalesce-1".to_string(),
        "https://www.linkedin.com/jobs/view/coalesce-3".to_string(),
    ]);
    assert_eq!(
        claimed_second,
        vec!["https://www.linkedin.com/jobs/view/coalesce-3".to_string()],
        "a URL already claimed by an in-flight pass must be excluded from a second pass's targets"
    );

    // Clean up so this test's state can't leak into another run of the suite.
    release_claim("https://www.linkedin.com/jobs/view/coalesce-1");
    release_claim("https://www.linkedin.com/jobs/view/coalesce-2");
    release_claim("https://www.linkedin.com/jobs/view/coalesce-3");
}

#[test]
fn releasing_a_url_lets_a_later_pass_reclaim_it() {
    let url = "https://www.linkedin.com/jobs/view/coalesce-release".to_string();

    let first = claim_unclaimed(vec![url.clone()]);
    assert_eq!(first, vec![url.clone()]);

    // While still claimed, a second pass sees nothing available.
    let while_in_flight = claim_unclaimed(vec![url.clone()]);
    assert!(
        while_in_flight.is_empty(),
        "the URL is still in flight and must not be claimable a second time"
    );

    release_claim(&url);

    // Once released (the first pass's fetch+write-back finished, success or
    // failure), a fresh pass can claim the same URL again.
    let after_release = claim_unclaimed(vec![url.clone()]);
    assert_eq!(
        after_release,
        vec![url.clone()],
        "a released URL must be claimable again by a later pass"
    );

    release_claim(&url);
}

#[test]
fn empty_input_claims_nothing() {
    assert!(claim_unclaimed(Vec::new()).is_empty());
}
