//! Persisting a verified availability (#1444): scoped to the exact posting url.

use super::support::*;
use crate::scraping::availability::{classify, Availability};

#[test]
fn closure_is_stored_on_the_exact_url_and_a_repost_does_not_inherit_it() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP", "linkedin", 0.0, "manual");
    let old = "https://www.linkedin.com/jobs/view/111";
    let repost = "https://www.linkedin.com/jobs/view/222";
    record(
        &store,
        &ap.id,
        1,
        vec![
            found_job_full(old, "Engineer", "Acme", 1),
            found_job_full(repost, "Engineer", "Acme", 2),
        ],
    );

    let closed = classify(Some(200), "No longer accepting applications");
    let n = store.set_availability(&crate::applications::normalize_job_url(old), closed, 99);
    assert_eq!(n, 1);

    let jobs = store.get(&ap.id).unwrap().found_jobs;
    let by = |u: &str| jobs.iter().find(|j| j.url == u).unwrap().clone();
    assert_eq!(by(old).availability, Some(Availability::Closed));
    assert_eq!(by(old).availability_checked_at, Some(99));
    assert_eq!(by(repost).availability, None, "repost reads as unchecked");
}
