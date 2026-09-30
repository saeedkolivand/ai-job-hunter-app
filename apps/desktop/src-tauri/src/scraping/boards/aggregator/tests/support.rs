//! Shared fixtures for the aggregator test topics: `FakeProvider` /
//! `RecordingProvider` fakes, small builders (`make_token`, `sample_posting`,
//! `make_input`, `make_ctx`), the Adzuna wiremock helpers, and the
//! keyring-slot fixtures serialized on `AGG_KEYRING_LOCK`.

use std::sync::Mutex;

use super::super::*;
use crate::ipc_contracts::provider_slots::{
    ADZUNA_APP_ID, ADZUNA_APP_KEY, APIFY_TOKEN, JOOBLE_KEY, JSEARCH_KEY,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(super) fn make_token() -> tokio_util::sync::CancellationToken {
    tokio_util::sync::CancellationToken::new()
}

pub(super) fn sample_posting(id: &str, provider: &str) -> JobPosting {
    JobPosting {
        id: format!("aggregator:{provider}-{id}"),
        external_id: Some(format!("{provider}-{id}")),
        title: format!("Engineer {id}"),
        company: "Acme".to_string(),
        location: Some("Berlin, DE".to_string()),
        url: format!("https://{provider}.example.com/job/{id}"),
        source: "aggregator".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    }
}

// ── Fake providers ────────────────────────────────────────────────────────────

pub(super) struct FakeProvider {
    id: &'static str,
    configured: bool,
    result: Result<Vec<JobPosting>, &'static str>,
}

impl FakeProvider {
    pub(super) fn ok(id: &'static str, items: Vec<JobPosting>) -> Self {
        Self {
            id,
            configured: true,
            result: Ok(items),
        }
    }

    pub(super) fn err(id: &'static str, msg: &'static str) -> Self {
        Self {
            id,
            configured: true,
            result: Err(msg),
        }
    }

    pub(super) fn unconfigured(id: &'static str) -> Self {
        Self {
            id,
            configured: false,
            result: Ok(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl JobProvider for FakeProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    async fn search(
        &self,
        _query: &str,
        _location: &str,
        _country: &str,
        _country_guessed: bool,
        _date_filter: Option<&str>,
        _amount: Option<u32>,
        _signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        match &self.result {
            Ok(v) => Ok(v.clone()),
            Err(msg) => Err(anyhow::anyhow!(*msg)),
        }
    }
}

/// Records the per-call upstream SPEND CAP (`amount`) each call was handed, and
/// whether it was called at all.
///
/// [`FakeProvider`] discards that argument, which is exactly why no existing test
/// could see the paid tier's cap — every Apify test asserted on merged OUTPUT and
/// would have stayed green while the gate bought a full 50-item actor run on every
/// scheduled search. Cost assertions need the argument, not the result.
pub(super) struct RecordingProvider {
    id: &'static str,
    items: Vec<JobPosting>,
    calls: CallLog,
}

/// Spend caps observed by a [`RecordingProvider`], newest last. `None` entries are
/// calls made with no cap at all.
pub(super) type CallLog = std::sync::Arc<std::sync::Mutex<Vec<Option<u32>>>>;

impl RecordingProvider {
    pub(super) fn new(id: &'static str, items: Vec<JobPosting>) -> (Self, CallLog) {
        let calls: CallLog = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            Self {
                id,
                items,
                calls: calls.clone(),
            },
            calls,
        )
    }
}

#[async_trait::async_trait]
impl JobProvider for RecordingProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn is_configured(&self) -> bool {
        true
    }

    async fn search(
        &self,
        _query: &str,
        _location: &str,
        _country: &str,
        _country_guessed: bool,
        _date_filter: Option<&str>,
        amount: Option<u32>,
        _signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        self.calls.lock().expect("recorder mutex").push(amount);
        Ok(self.items.clone())
    }
}

// ── Apify LinkedIn provider fixture ──────────────────────────────────────────

/// A fully-formed Apify provider with explicit fields (no keyring read).
pub(super) fn apify(token: Option<&str>, enabled: bool) -> ApifyLinkedInProvider {
    ApifyLinkedInProvider {
        token: token.map(str::to_string),
        enabled,
        actor_id: APIFY_DEFAULT_ACTOR.to_string(),
    }
}

// ── BoardSearchInput / ScrapeContext builders (needs-keys classification) ────

pub(super) fn make_input() -> BoardSearchInput {
    BoardSearchInput {
        query: "engineer".into(),
        location: Some("berlin".into()),
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: Some("de".into()),
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec![],
    }
}

pub(super) fn make_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: make_token(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    }
}

// ── Adzuna wiremock request/response builders (page-loop + quota-neutral tests) ──

/// One Adzuna response page carrying `count` results with sequential ids offset
/// by `first_id`. Only the three non-optional `AdzunaJob` fields are populated —
/// the rest are `Option` and legitimately absent from real sparse rows.
pub(super) fn adzuna_body(first_id: u32, count: u32) -> serde_json::Value {
    let results: Vec<serde_json::Value> = (first_id..first_id + count)
        .map(|i| {
            serde_json::json!({
                "id": i,
                "title": format!("Engineer {i}"),
                "redirect_url": format!("https://example.test/job/{i}"),
            })
        })
        .collect();
    serde_json::json!({ "count": results.len(), "results": results })
}

pub(super) fn adzuna_req(base_url: &str) -> AdzunaPageRequest<'_> {
    AdzunaPageRequest {
        base_url,
        country: "de",
        app_id: "fake-id",
        app_key: "fake-key",
        query: "engineer",
        where_val: "Berlin",
        date_filter: None,
    }
}

/// Adzuna's page number is a 1-based PATH segment: `…/search/{page}`.
pub(super) fn adzuna_path(page: u32) -> String {
    format!("/v1/api/jobs/de/search/{page}")
}

/// An `AdzunaProvider` with fake credentials pointed at a local `wiremock` host.
/// The single place the provider is hand-built, so a new field on the struct
/// breaks one line instead of every wiremock test.
pub(super) fn wiremock_adzuna(
    base_url: String,
    note_sink: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
) -> AdzunaProvider {
    AdzunaProvider {
        app_id: Some("fake-id".to_string()),
        app_key: Some("fake-key".to_string()),
        note_sink,
        base_url: ADZUNA_BASE_URL.to_string(),
    }
    .with_base_url(base_url)
}

// ── Keyring-slot fixtures (credential-read degradation + needs-keys tests) ───

pub(super) static AGG_KEYRING_LOCK: Mutex<()> = Mutex::new(());

/// The aggregator's `ai:`-namespaced keyring slots, built from the generated
/// bare slot consts so the test asserts against the single cross-language source
/// of truth (drift in `provider_slots` flows straight through here).
pub(super) fn adzuna_slots() -> [String; 2] {
    [
        format!("ai:{ADZUNA_APP_ID}"),
        format!("ai:{ADZUNA_APP_KEY}"),
    ]
}

pub(super) fn jsearch_slot() -> String {
    format!("ai:{JSEARCH_KEY}")
}

pub(super) fn apify_slot() -> String {
    format!("ai:{APIFY_TOKEN}")
}

pub(super) fn jooble_slot() -> String {
    format!("ai:{JOOBLE_KEY}")
}

/// Delete the aggregator's fixed keyring slots so a test starts from a known
/// "absent" baseline regardless of what a previous serialized test left behind.
pub(super) fn clear_aggregator_slots() {
    let adzuna = adzuna_slots();
    let jsearch = jsearch_slot();
    let apify = apify_slot();
    let jooble = jooble_slot();
    for slot in adzuna
        .iter()
        .chain(std::iter::once(&jsearch))
        .chain(std::iter::once(&apify))
        .chain(std::iter::once(&jooble))
    {
        if let Ok(entry) = keyring_core::Entry::new(crate::credentials::SERVICE, slot) {
            // NoEntry on a clean slot is fine; we only care it ends up absent.
            let _ = entry.delete_credential();
        }
    }
}
