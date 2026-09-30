use super::super::*;
use super::support::*;

// ── Finding #1: Apify URL validation ─────────────────────────────────────────

/// A `jobUrl` on a non-LinkedIn host is rejected — the item is dropped.
#[test]
fn apify_map_item_rejects_non_linkedin_host() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "https://evil.example.com/jobs/1"
    }))
    .unwrap();
    assert!(
        map_apify_item(item, 0).is_none(),
        "non-LinkedIn jobUrl must be rejected"
    );
}

/// `http://` (not https) and `javascript:` schemes must be rejected.
#[test]
fn apify_map_item_rejects_non_https_scheme() {
    let item_http: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "http://www.linkedin.com/jobs/view/1"
    }))
    .unwrap();
    assert!(
        map_apify_item(item_http, 0).is_none(),
        "http:// LinkedIn URL must be rejected (https required)"
    );

    let item_js: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "javascript:alert(1)"
    }))
    .unwrap();
    assert!(
        map_apify_item(item_js, 0).is_none(),
        "javascript: scheme must be rejected"
    );
}

/// A non-numeric `id` (e.g. a path-traversal string) must not be used to
/// construct a URL — the item is dropped because no safe URL can be built.
#[test]
fn apify_map_item_rejects_non_numeric_id_for_url_construction() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "id": "../../etc/passwd"
    }))
    .unwrap();
    // Non-numeric id → URL cannot be constructed → item dropped (None).
    assert!(
        map_apify_item(item, 0).is_none(),
        "non-numeric id must not be used to construct a LinkedIn URL; item must be dropped"
    );
}

/// A valid HTTPS `linkedin.com` URL passes validation and produces a JobPosting.
#[test]
fn apify_map_item_accepts_valid_linkedin_url() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "https://www.linkedin.com/jobs/view/99999"
    }))
    .unwrap();
    let p = map_apify_item(item, 0).expect("valid LinkedIn HTTPS URL must be accepted");
    assert_eq!(p.url, "https://www.linkedin.com/jobs/view/99999");
}

// ── Finding #2: cost gate — skip Apify when primary fills amount ──────────────

/// Primary provides exactly `amount` items → the paid Apify call must NOT fire.
/// Uses a `TrackCallProvider` (not FakeProvider::err) because Apify errors are
/// silently swallowed — only a flag proves the call was skipped.
#[tokio::test]
async fn apify_skipped_when_primary_fills_amount() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct TrackCallProvider {
        id: &'static str,
        called: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl JobProvider for TrackCallProvider {
        fn provider_id(&self) -> &'static str {
            self.id
        }
        fn is_configured(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: bool,
            _: Option<&str>,
            _: Option<u32>,
            _: tokio_util::sync::CancellationToken,
        ) -> anyhow::Result<Vec<JobPosting>> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![])
        }
    }

    let apify_called = Arc::new(AtomicBool::new(false));
    let primary_items: Vec<JobPosting> = (0..5_u32)
        .map(|i| sample_posting(&i.to_string(), "adzuna"))
        .collect();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", primary_items.clone())),
        Box::new(TrackCallProvider {
            id: "apify_linkedin",
            called: apify_called.clone(),
        }),
    ];

    // amount == 5, primary returns 5 → Apify must not be called.
    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(5),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 5, "primary result must be returned unchanged");
    assert!(
        !apify_called.load(Ordering::SeqCst),
        "Apify must NOT be called when primary already fills the requested amount"
    );
}

/// `build_apify_endpoint` reflects the dynamic cap in its `maxItems` query param.
/// - Partial gap (remaining=20): endpoint has `maxItems=20`.
/// - Full budget (remaining≥APIFY_MAX_ITEMS): endpoint has `maxItems=APIFY_MAX_ITEMS`.
#[test]
fn apify_endpoint_cap_reflects_remaining() {
    // Partial: amount=30, primary=10, remaining=20 → cap=20.
    let ep_partial = build_apify_endpoint(APIFY_DEFAULT_ACTOR, 20);
    assert!(
        ep_partial.contains("maxItems=20"),
        "partial-gap cap must appear in maxItems; got: {ep_partial}"
    );

    // Full: remaining exceeds APIFY_MAX_ITEMS → cap clamped to APIFY_MAX_ITEMS.
    let ep_full = build_apify_endpoint(APIFY_DEFAULT_ACTOR, APIFY_MAX_ITEMS);
    assert!(
        ep_full.contains(&format!("maxItems={APIFY_MAX_ITEMS}")),
        "full-cap scenario must use APIFY_MAX_ITEMS; got: {ep_full}"
    );
}

// ── Finding #3: canonical_url preserves query case for non-LinkedIn URLs ──────

/// Two non-LinkedIn URLs differing ONLY by query-string case must NOT collapse
/// to the same dedup key.  Some boards encode job ids as case-sensitive query
/// params; the old `.to_lowercase()` on the whole URL would merge them silently.
#[test]
fn canonical_url_non_linkedin_query_case_is_preserved() {
    let key1 = canonical_url("https://board.example.com/jobs?ref=AbCdEf");
    let key2 = canonical_url("https://board.example.com/jobs?ref=abcdef");
    assert_ne!(
        key1, key2,
        "non-LinkedIn URLs differing only by query case must remain distinct; \
         both canonicalized to: {key1}"
    );
}
