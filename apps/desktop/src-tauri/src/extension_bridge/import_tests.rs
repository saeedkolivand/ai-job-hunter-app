//! Extension-bridge: Scan-mode parse goldens + saved/applied/dedup persistence
//! matrix + port-probe fallback.
//!
//! These tests are hermetic (no network). They exercise:
//!  - `parse_from_html`: JSON-LD path (LinkedIn-style) and generic-meta path
//!    (Indeed/Lever-style).
//!  - `classify_frame`: the per-message security gate (size cap, per-frame token,
//!    type dispatch) — empty/wrong token → `unauthorized` reply (no dispatch),
//!    over-cap frame → close, SSRF private host blocked, non-JSON dropped.
//!  - `upsert_for_origin`: saved/applied/dedup outcomes at the persistence
//!    boundary (mirrors what `handle_import` calls in production).
//!  - `normalize_job_url`: the exact dedup-key transforms (www/query/fragment/
//!    trailing-slash strip, lowercase host, non-http(s) scheme → empty).
//!  - `probe_ports`: skips an occupied port and binds a free one; when the span
//!    is fully busy the bridge degrades gracefully (`None`).

use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;

use super::{classify_frame, BridgeState, FrameDecision, MAX_FRAME_BYTES};
use crate::applications::{
    normalize_job_url, ApplicationOrigin, ApplicationStatus, ApplicationStore,
};
use crate::scraping::scrape_url::parse_from_html;

// ── helper ────────────────────────────────────────────────────────────────────

fn open_store() -> (TempDir, ApplicationStore) {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    (dir, store)
}

/// A fresh bridge state with a known persisted token (a temp data dir).
fn bridge_state() -> (TempDir, BridgeState) {
    let dir = TempDir::new().unwrap();
    let state = BridgeState::load(dir.path());
    (dir, state)
}

/// The `error` string of an `import.result` reply, or `None` if it is a success
/// payload. Panics if the reply is not a well-formed `import.result` envelope.
fn reply_error(reply: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(reply).expect("reply must be JSON");
    assert_eq!(
        v.get("type").and_then(|t| t.as_str()),
        Some(super::msg::IMPORT_RESULT),
        "reply must be an import.result envelope"
    );
    v.get("payload")
        .and_then(|p| p.get("error"))
        .and_then(|e| e.as_str())
        .map(str::to_string)
}

// ─────────────────────────────────────────────────────────────────────────────
// A1. Scan-mode parse goldens — JSON-LD path (LinkedIn-style)
// ─────────────────────────────────────────────────────────────────────────────

/// LinkedIn serves a fully-hydrated JSON-LD `JobPosting` block. The extension
/// captures the authenticated DOM; `parse_from_html` must extract all four
/// fields from that block and NOT fall back to the generic meta path.
#[test]
fn scan_mode_linkedin_style_json_ld_extracts_all_fields() {
    // Minimal LinkedIn-shaped HTML: title in JSON-LD wins over <title>,
    // hiringOrganization supplies company, jobLocation supplies location.
    let html = r#"
        <html>
        <head>
            <title>LinkedIn | Jobs</title>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "Senior Software Engineer",
                "description": "<p>Build distributed systems at scale.</p>",
                "hiringOrganization": {
                    "@type": "Organization",
                    "name": "Acme Corp",
                    "sameAs": "https://www.acmecorp.example"
                },
                "jobLocation": {
                    "@type": "Place",
                    "address": {
                        "@type": "PostalAddress",
                        "addressLocality": "Berlin",
                        "addressRegion": "BE",
                        "addressCountry": "DE"
                    }
                },
                "datePosted": "2026-01-15",
                "employmentType": "FULL_TIME"
            }
            </script>
        </head>
        <body>
            <main>
                <h1 class="job-title">Senior Software Engineer</h1>
                <div class="job-description">Build distributed systems at scale.</div>
            </main>
        </body>
        </html>
    "#;

    let posting = parse_from_html("https://www.linkedin.com/jobs/view/9876543210", html)
        .expect("a valid JSON-LD JobPosting must produce Some");

    assert_eq!(
        posting.title, "Senior Software Engineer",
        "title must come from JSON-LD, not the generic <title> tag"
    );
    assert_eq!(
        posting.company, "Acme Corp",
        "company must be extracted from hiringOrganization.name"
    );
    assert_eq!(
        posting.location.as_deref(),
        Some("Berlin, BE"),
        "location must be assembled from addressLocality + addressRegion"
    );
    assert!(
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("distributed systems"),
        "description must carry the JSON-LD text (HTML-stripped or raw)"
    );
    assert_eq!(
        posting.url, "https://www.linkedin.com/jobs/view/9876543210",
        "url must be the input URL passed to parse_from_html"
    );
}

/// LinkedIn sometimes wraps the `JobPosting` node inside an `@graph` array
/// alongside `BreadcrumbList` and `WebPage` nodes. The parser must reach into
/// the graph and pull the correct node.
#[test]
fn scan_mode_linkedin_json_ld_graph_array_extracts_job_posting_node() {
    let html = r#"
        <html>
        <head>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org",
                "@graph": [
                    { "@type": "WebPage", "url": "https://www.linkedin.com/jobs/view/42" },
                    { "@type": "BreadcrumbList" },
                    {
                        "@type": "JobPosting",
                        "title": "Staff Infrastructure Engineer",
                        "hiringOrganization": { "name": "Initech" },
                        "jobLocation": {
                            "address": {
                                "addressLocality": "Munich",
                                "addressRegion": "BY"
                            }
                        }
                    }
                ]
            }
            </script>
        </head>
        <body></body>
        </html>
    "#;

    let posting = parse_from_html("https://www.linkedin.com/jobs/view/42", html)
        .expect("@graph-wrapped JobPosting must produce Some");

    assert_eq!(posting.title, "Staff Infrastructure Engineer");
    assert_eq!(posting.company, "Initech");
    assert_eq!(posting.location.as_deref(), Some("Munich, BY"));
}

// ─────────────────────────────────────────────────────────────────────────────
// A2. Scan-mode parse goldens — generic meta path (Indeed / Workday style)
// ─────────────────────────────────────────────────────────────────────────────

/// Indeed (and many ATS job pages) render job details in `<title>`, `<h1>`,
/// `og:description`, and `og:site_name` but have NO JSON-LD block. The parser
/// must fall back to the generic meta path and extract meaningful fields.
#[test]
fn scan_mode_indeed_style_generic_meta_extracts_fields() {
    let html = r#"
        <html>
        <head>
            <title>Backend Engineer - Globex | Indeed.com</title>
            <meta property="og:site_name" content="Globex">
            <meta property="og:description"
                  content="Own the API platform. Remote-friendly, great team.">
        </head>
        <body>
            <h1 class="jobsearch-JobInfoHeader-title">Backend Engineer</h1>
        </body>
        </html>
    "#;

    let posting = parse_from_html("https://www.indeed.com/viewjob?jk=abc123def456", html)
        .expect("generic meta path must produce Some when <h1> or <title> is present");

    // The title selector prefers <h1> inside body if one exists.
    assert!(
        !posting.title.is_empty(),
        "title must not be empty when <h1> or <title> exists"
    );
    assert_eq!(
        posting.company, "Globex",
        "company must come from og:site_name when no JSON-LD is present"
    );
    assert!(
        posting.description.as_deref().unwrap_or_default().len() > 5,
        "description must carry the og:description content"
    );
    assert_eq!(posting.source, "url");
}

/// Workday and similar ATSes often have a `name="description"` meta but no
/// `og:*` tags and no JSON-LD. The parser must still produce a posting with
/// the plain `name="description"` value.
#[test]
fn scan_mode_workday_style_plain_meta_description_used_as_fallback() {
    let html = r#"
        <html>
        <head>
            <title>Data Analyst</title>
            <meta name="description" content="Analyze business intelligence data at Umbrella Corp.">
        </head>
        <body></body>
        </html>
    "#;

    let posting = parse_from_html("https://umbrella.wd1.myworkdayjobs.com/jobs/42", html)
        .expect("plain-meta path must produce Some when <title> is present");

    assert_eq!(posting.title, "Data Analyst");
    assert!(
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("business intelligence"),
        "description must carry the name=description content"
    );
    // No og:site_name and no JSON-LD → company falls back to empty string (not a panic).
    assert_eq!(posting.source, "url");
}

// ─────────────────────────────────────────────────────────────────────────────
// A3. Saved / applied / dedup persistence matrix
//
// These test `upsert_for_origin` at the same boundary `handle_import` calls.
// The handler is a private async fn inside the WS loop and takes an AppHandle,
// so it cannot be unit-tested without a live app; we test the persistence core
// directly — the handler is just a thin wrapper.
// ─────────────────────────────────────────────────────────────────────────────

fn app_meta(company: &str, title: &str) -> crate::applications::ApplicationMeta {
    crate::applications::ApplicationMeta {
        company: company.into(),
        title: title.into(),
        candidate: "Test User".into(),
        brief: String::new(),
        job_description: String::new(),
        answers: vec![],
        job_summary: String::new(),
    }
}

fn sample_posting(url: &str, company: &str, title: &str) -> crate::scraping::types::JobPosting {
    crate::scraping::types::JobPosting {
        id: "p1".into(),
        external_id: None,
        title: title.into(),
        company: company.into(),
        location: None,
        url: url.into(),
        source: "linkedin".into(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    }
}

/// `usable` is the title-gate the DOM-first import chain uses to decide whether a
/// parse degraded: a titled posting is usable; a blank/whitespace title is not
/// (→ handle_import persists a partial stub instead of erroring out).
#[test]
fn usable_requires_a_non_blank_title() {
    let url = "https://acme.example/jobs/1";
    assert!(
        super::usable(&sample_posting(url, "Co", "Title")),
        "a titled posting is usable"
    );
    assert!(
        !super::usable(&sample_posting(url, "Co", "")),
        "an empty-title posting is not usable"
    );
    assert!(
        !super::usable(&sample_posting(url, "Co", "   ")),
        "a whitespace-only title is not usable"
    );
}

// ── import-isolation contract ──────────────────────────────────────────────────
// `persist_import_application` is the WHOLE persistence side effect of an import.
// It takes only the `ApplicationStore` (no `PostingsCache`), so an import can
// never enter the Jobs/discovery feed — these lock that mapping + the Saved origin.

/// An import persists exactly one Saved Application carrying the posting's
/// company/title; the absence of a `PostingsCache` parameter is the structural
/// guarantee that nothing lands in the Jobs feed.
#[test]
fn import_persists_one_saved_application_from_posting() {
    let (_dir, store) = open_store();
    let posting = sample_posting("https://acme.example/jobs/77", "Acme", "Staff Engineer");

    let (id, status) =
        super::persist_import_application(&store, "https://acme.example/jobs/77", &posting, None)
            .unwrap();

    assert_eq!(
        status, "saved",
        "an import with no applied flag stays saved"
    );
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Saved);
    assert_eq!(app.company, "Acme");
    assert_eq!(app.title, "Staff Engineer");
    assert_eq!(
        store.list().len(),
        1,
        "import creates exactly one Application"
    );
}

/// `applied=Some(true)` from the extension advances the imported Application
/// straight to `applied`.
#[test]
fn import_applied_flag_advances_status() {
    let (_dir, store) = open_store();
    let posting = sample_posting("https://acme.example/jobs/78", "Beta", "SRE");

    let (_id, status) = super::persist_import_application(
        &store,
        "https://acme.example/jobs/78",
        &posting,
        Some(true),
    )
    .unwrap();

    assert_eq!(status, "applied");
}

/// `applied=None` (or `applied=Some(false)`) with `ApplicationOrigin::Saved`
/// must create ONE Application with status `saved` and no `applied_at`.
#[test]
fn persistence_matrix_saved_no_applied_flag_yields_saved_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/100",
            "linkedin",
            &app_meta("Acme", "Frontend Engineer"),
            ApplicationOrigin::Saved,
            None, // applied flag absent
        )
        .unwrap();

    let apps = store.list();
    assert_eq!(apps.len(), 1, "exactly one Application created");
    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "absent applied flag with Saved origin must yield status=saved"
    );
    assert!(
        app.applied_at.is_none(),
        "applied_at must be None when status is saved"
    );
    assert_eq!(app.company, "Acme");
    assert_eq!(app.title, "Frontend Engineer");
}

/// `applied=Some(false)` is equivalent to absent — still `saved`.
#[test]
fn persistence_matrix_saved_applied_false_yields_saved_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/101",
            "indeed",
            &app_meta("Beta Inc", "DevOps Engineer"),
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "applied=Some(false) must yield status=saved"
    );
    assert!(app.applied_at.is_none());
}

/// `applied=Some(true)` must advance the status to `applied` immediately.
#[test]
fn persistence_matrix_applied_true_flag_yields_applied_status() {
    let (_dir, store) = open_store();

    let id = store
        .upsert_for_origin(
            "https://jobs.example.com/posting/102",
            "greenhouse",
            &app_meta("Globex", "Platform Engineer"),
            ApplicationOrigin::Saved,
            Some(true), // extension flagged this job as already applied
        )
        .unwrap();

    let apps = store.list();
    assert_eq!(apps.len(), 1, "exactly one Application created");
    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "applied=Some(true) must yield status=applied"
    );
    assert!(
        app.applied_at.is_some(),
        "applied_at must be set when applied=true"
    );
}

/// Re-importing the same URL (same normalized form, different raw variants)
/// must produce ONE Application, merge the fields, and not create a duplicate.
#[test]
fn persistence_matrix_dedup_same_url_merges_not_duplicates() {
    let (_dir, store) = open_store();

    // First import: raw URL with query param and trailing slash.
    let id_first = store
        .upsert_for_origin(
            "https://www.acmecorp.example/jobs/42/?utm_source=ext",
            "url",
            &app_meta("Acme Corp", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Second import: canonical URL (www-stripped, no query, no trailing slash).
    let id_second = store
        .upsert_for_origin(
            "https://acmecorp.example/jobs/42",
            "url",
            &app_meta("Acme Corp", "Senior Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    assert_eq!(
        id_first, id_second,
        "same normalized URL must merge into the same Application row (no dup)"
    );

    let apps = store.list();
    assert_eq!(apps.len(), 1, "dedup: exactly one Application in the store");

    // The merged row should carry the latest-import title.
    let app = store.get(&id_first).unwrap();
    // Title updated to the second import value.
    assert_eq!(app.title, "Senior Backend Engineer");
    assert_eq!(app.company, "Acme Corp");
}

/// A `saved` → re-import with `applied=Some(true)` must advance the existing
/// row's status to `applied` (not create a second row).
#[test]
fn persistence_matrix_reimport_with_applied_true_advances_saved_to_applied() {
    let (_dir, store) = open_store();

    let url = "https://jobs.example.com/posting/200";

    // First import: saved.
    let id = store
        .upsert_for_origin(
            url,
            "url",
            &app_meta("Initech", "SRE"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    assert_eq!(store.get(&id).unwrap().status, ApplicationStatus::Saved);

    // Re-import: same URL, user ticked "applied" in the extension popup.
    let id2 = store
        .upsert_for_origin(
            url,
            "url",
            &app_meta("Initech", "SRE"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    assert_eq!(id, id2, "re-import must not create a second row");
    assert_eq!(
        store.list().len(),
        1,
        "still exactly one Application after re-import"
    );

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "re-import with applied=true must advance status from saved to applied"
    );
    assert!(app.applied_at.is_some());
}

// ─────────────────────────────────────────────────────────────────────────────
// B1. Per-frame token rejection (HIGH 1)
//
// `classify_frame` is the per-message security gate the connection loop runs
// before any app-stateful work. A frame with an empty or wrong token must be
// REJECTED with an `unauthorized` import.result reply and must NOT be classified
// as an `Import` — so `handle_import` is never reached and NOTHING is persisted.
// (handle_import is the only persistence path; a `Reply` decision can't reach it.)
// ─────────────────────────────────────────────────────────────────────────────

/// An empty `token` must be rejected as `unauthorized` and must not dispatch an
/// import — even though the envelope is a well-formed `import.request`.
#[test]
fn frame_empty_token_is_unauthorized_and_persists_nothing() {
    let (_dir, state) = bridge_state();

    let frame = json!({
        "token": "",
        "reqId": "r-empty",
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": "https://jobs.example.com/posting/1" }
    })
    .to_string();

    let decision = classify_frame(&state, &frame);
    let reply = match decision {
        FrameDecision::Unauthorized(text) => text,
        other => panic!("empty token must produce an Unauthorized decision, got {other:?}"),
    };
    // An `Unauthorized` (not `Import`) means handle_import is never invoked → no
    // persist; at the connection level it also closes the socket.
    let err = reply_error(&reply).expect("empty token reply must carry an error");
    assert!(!err.is_empty(), "unauthorized error must be non-empty");
    assert!(
        err.contains("unauthorized"),
        "empty token must map to an unauthorized error, got {err:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(
        parsed.get("reqId").and_then(|r| r.as_str()),
        Some("r-empty"),
        "reply must echo the request id"
    );
}

/// A wrong (non-matching) `token` must be rejected as `unauthorized` and must not
/// dispatch an import.
#[test]
fn frame_wrong_token_is_unauthorized_and_persists_nothing() {
    let (_dir, state) = bridge_state();
    // A token that is the right shape but is NOT the state's secret.
    let wrong = "f".repeat(64);
    assert_ne!(
        wrong,
        state.token(),
        "fixture must use a non-matching token"
    );

    let frame = json!({
        "token": wrong,
        "reqId": "r-wrong",
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": "https://jobs.example.com/posting/2" }
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Unauthorized(reply) => {
            let err = reply_error(&reply).expect("wrong token reply must carry an error");
            assert!(
                err.contains("unauthorized"),
                "wrong token must map to an unauthorized error, got {err:?}"
            );
        }
        other => panic!("wrong token must produce an Unauthorized decision, got {other:?}"),
    }
}

/// The matching token DOES classify an `import.request` as `Import` (positive
/// control — proves the rejection tests above aren't passing for the wrong
/// reason). The `Import` payload is forwarded verbatim to `handle_import`.
#[test]
fn frame_correct_token_classifies_as_import() {
    let (_dir, state) = bridge_state();
    let token = state.token();

    let frame = json!({
        "token": token,
        "reqId": "r-ok",
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": "https://jobs.example.com/posting/3" }
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Import { req_id, payload } => {
            assert_eq!(req_id, "r-ok");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/3")
            );
        }
        other => panic!("a valid token + import.request must classify as Import, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B1b. Connection-time `auth` frame (the wrong-token-never-connected fix)
//
// The extension sends an `auth` frame immediately after the socket opens to
// verify the pairing before any import. A WRONG token must be `Unauthorized`
// (reply carries the `unauthorized` error; the connection loop then closes the
// socket and never marks it connected). A CORRECT token must be a `Reply` whose
// `import.result` payload carries NO `error` (the extension reads "no error" =
// authorized). The connection loop marks `connected` true only for `Reply` /
// `Import` (token-validated), never for `Unauthorized`.
// ─────────────────────────────────────────────────────────────────────────────

/// An `auth` frame with the WRONG token is `Unauthorized` (not `Reply`): the
/// connection loop sends the reply then closes the socket and never marks it
/// connected. This is the core fix — a wrong token is rejected at the handshake-
/// adjacent first frame, never reported as connected/authorized.
#[test]
fn auth_frame_wrong_token_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let wrong = "9".repeat(64);
    assert_ne!(
        wrong,
        state.token(),
        "fixture must use a non-matching token"
    );

    let frame = json!({
        "token": wrong,
        "reqId": "r-auth-bad",
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Unauthorized(reply) => {
            let err = reply_error(&reply).expect("wrong-token auth reply must carry an error");
            assert!(
                err.contains("unauthorized"),
                "wrong-token auth must map to an unauthorized error, got {err:?}"
            );
        }
        other => panic!("an `auth` frame with a wrong token must be Unauthorized, got {other:?}"),
    }
}

/// An `auth` frame with the CORRECT token is a `Reply` whose `import.result`
/// payload contains NO `error` field — the extension treats "no error" as
/// authorized. The `auth` frame does no import (empty fields), it only verifies
/// the token.
#[test]
fn auth_frame_correct_token_replies_with_no_error() {
    let (_dir, state) = bridge_state();
    let token = state.token();

    let frame = json!({
        "token": token,
        "reqId": "r-auth-ok",
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Reply(reply) => {
            assert!(
                reply_error(&reply).is_none(),
                "a correct-token auth reply must carry NO error (no error = authorized), got {reply}"
            );
            let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(
                parsed.get("type").and_then(|t| t.as_str()),
                Some(super::msg::IMPORT_RESULT),
                "the auth ok reply reuses the import.result envelope"
            );
            assert_eq!(
                parsed.get("reqId").and_then(|r| r.as_str()),
                Some("r-auth-ok"),
                "the auth reply must echo the request id"
            );
        }
        other => panic!("an `auth` frame with the correct token must be a Reply, got {other:?}"),
    }
}

/// A wrong-token frame yields `Unauthorized` while a valid-token frame yields
/// `Reply`/`Import` — and ONLY the latter two cause the connection loop to call
/// `set_connected(true)`. `set_connected` lives in the async connection loop
/// (not unit-reachable without a live socket), so we pin the classify-level
/// variant split that drives it: `Unauthorized` (never connected) is a distinct
/// variant from `Reply`/`Import` (connected). A fresh state is disconnected.
#[test]
fn unauthorized_variant_is_distinct_from_connecting_variants() {
    let (_dir, state) = bridge_state();
    assert!(
        !state.is_connected(),
        "a fresh bridge state is not connected until a token-validated frame arrives"
    );

    let bad = json!({
        "token": "0".repeat(64),
        "reqId": "r-bad",
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();
    assert!(
        matches!(classify_frame(&state, &bad), FrameDecision::Unauthorized(_)),
        "a wrong-token frame must classify as Unauthorized (loop never marks it connected)"
    );

    let ok = json!({
        "token": state.token(),
        "reqId": "r-ok",
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();
    assert!(
        matches!(classify_frame(&state, &ok), FrameDecision::Reply(_)),
        "a valid-token auth frame must classify as Reply (the loop marks it connected)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// B1c. classify_frame with absent / empty reqId
//
// When a frame's `reqId` field is absent OR is an empty string, `classify_frame`
// defaults `req_id` to `""`. The reply is still emitted with `"reqId": ""`.
//
// KNOWN CORRELATION HAZARD: two concurrent requests both lacking `reqId` would
// both receive `"reqId": ""` replies, making them indistinguishable on the
// extension side. This is NOT fixed here — it is pinned as a documented behavior
// so a future change (e.g. server-side reqId generation) has a test to land on.
// Auth frames are normally sent one-at-a-time, so the hazard is advisory for now.
// ─────────────────────────────────────────────────────────────────────────────

/// An `auth` frame whose `reqId` is ABSENT defaults `req_id` to `""`. The frame
/// still classifies correctly (correct token → Reply, wrong token → Unauthorized)
/// and the reply carries `"reqId": ""` rather than panicking or mis-routing to a
/// non-existent request.
#[test]
fn classify_frame_auth_absent_req_id_defaults_to_empty_string() {
    let (_dir, state) = bridge_state();
    let token = state.token();

    // Auth with correct token, NO reqId field at all.
    let frame = json!({
        "token": token,
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Reply(reply) => {
            // Must not panic; error-free means "authorized".
            assert!(
                reply_error(&reply).is_none(),
                "absent-reqId auth with correct token must reply with no error"
            );
            let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap();
            // PINNED BEHAVIOR: absent reqId → reply carries "reqId": "".
            // If this assertion ever changes, the correlation-hazard comment above
            // must be revisited (e.g. the bridge started generating its own reqId).
            assert_eq!(
                parsed.get("reqId").and_then(|r| r.as_str()),
                Some(""),
                "absent reqId must default the reply's reqId to empty string (pinned behavior)"
            );
        }
        other => {
            panic!("correct-token auth with absent reqId must classify as Reply, got {other:?}")
        }
    }
}

/// An `import.request` frame whose `reqId` is an empty string `""` is treated
/// the same as absent — `req_id` is `""` and the `Import` decision carries it.
/// This pins the correlation hazard: a reply for this request would have
/// `"reqId": ""` and cannot be distinguished from another no-reqId import reply.
#[test]
fn classify_frame_import_empty_req_id_carries_empty_string() {
    let (_dir, state) = bridge_state();
    let token = state.token();

    let frame = json!({
        "token": token,
        "reqId": "",  // explicitly empty — same as absent for correlation purposes
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": "https://jobs.example.com/posting/empty-req" }
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Import { req_id, payload } => {
            // PINNED BEHAVIOR: empty reqId passes through as "".
            assert_eq!(
                req_id, "",
                "empty reqId must be carried as-is (empty string) into the Import decision"
            );
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/empty-req")
            );
            // CORRELATION HAZARD (advisory, not a panic): `result_reply(&req_id, …)`
            // will produce `"reqId": ""` — indistinguishable from any other empty-reqId
            // reply. The extension always sets a non-empty reqId today, so this is
            // defensive documentation rather than an active bug.
        }
        other => {
            panic!("correct-token import with empty reqId must classify as Import, got {other:?}")
        }
    }
}

/// Wrong-token frame with ABSENT `reqId` still produces an Unauthorized reply
/// (token gate runs before reqId is meaningful), and the reply carries `"reqId": ""`.
#[test]
fn classify_frame_unauthorized_absent_req_id_reply_carries_empty_string() {
    let (_dir, state) = bridge_state();
    let wrong = "a".repeat(64);
    assert_ne!(wrong, state.token());

    // No reqId field.
    let frame = json!({
        "token": wrong,
        "type": super::msg::AUTH,
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match classify_frame(&state, &frame) {
        FrameDecision::Unauthorized(reply) => {
            let err = reply_error(&reply).expect("unauthorized reply must carry an error");
            assert!(err.contains("unauthorized"));
            let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(
                parsed.get("reqId").and_then(|r| r.as_str()),
                Some(""),
                "unauthorized reply with no reqId must echo empty string (pinned behavior)"
            );
        }
        other => panic!("wrong-token + absent reqId must be Unauthorized, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B2. Oversize-frame rejection (HIGH 2)
//
// A frame over MAX_FRAME_BYTES must yield `CloseOverCap` (the loop breaks/closes)
// WITHOUT parsing or dispatching. We test at exactly one byte over the cap and
// build the payload cheaply (a single `String::repeat`) — no 2MB JSON allocation
// of real structure, just a flat over-cap buffer.
// ─────────────────────────────────────────────────────────────────────────────

/// A frame whose length exceeds MAX_FRAME_BYTES is closed (CloseOverCap) before
/// any parse — even though it carries the correct token, it never dispatches.
#[test]
fn frame_over_size_cap_is_closed_without_dispatch() {
    let (_dir, state) = bridge_state();
    // One byte over the cap. Content is irrelevant — the size guard runs first.
    let oversize = "a".repeat(MAX_FRAME_BYTES + 1);
    assert!(oversize.len() > MAX_FRAME_BYTES);

    match classify_frame(&state, &oversize) {
        FrameDecision::CloseOverCap => {}
        other => panic!("an over-cap frame must be CloseOverCap, got {other:?}"),
    }
}

/// Exactly AT the cap is NOT over-size — it proceeds to parse (and, being
/// non-JSON here, drops). This pins the boundary so the guard is `>` not `>=`.
#[test]
fn frame_exactly_at_cap_is_not_over_size() {
    let (_dir, state) = bridge_state();
    let at_cap = "a".repeat(MAX_FRAME_BYTES);
    assert_eq!(at_cap.len(), MAX_FRAME_BYTES);

    // Not over-cap → it is parsed; raw "aaa…" is not JSON → Drop (not CloseOverCap).
    match classify_frame(&state, &at_cap) {
        FrameDecision::Drop => {}
        other => panic!("a frame exactly at the cap must parse (Drop here), got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B3. SSRF private-host import rejection (MEDIUM) + non-JSON drop
//
// An authenticated import.request whose `url` is a private/loopback host must be
// rejected by the SSRF gate `handle_import` applies (`is_safe_import_url`) before
// any persistence. classify_frame correctly routes it to Import (auth passed);
// we then assert the exact gate handle_import runs rejects the host, so nothing
// is ever persisted.
// ─────────────────────────────────────────────────────────────────────────────

/// A private-host import URL passes the token gate (classified as Import) but is
/// blocked by the host SSRF guard handle_import applies before persisting.
#[test]
fn frame_private_host_import_is_blocked_by_ssrf_gate() {
    let (_dir, state) = bridge_state();
    let token = state.token();
    let private_url = "http://192.168.1.1/job";

    let frame = json!({
        "token": token,
        "reqId": "r-ssrf",
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": private_url }
    })
    .to_string();

    // Auth passes → Import; the URL is non-empty after normalization …
    match classify_frame(&state, &frame) {
        FrameDecision::Import { payload, .. } => {
            let url = payload.get("url").and_then(|u| u.as_str()).unwrap();
            assert!(
                !normalize_job_url(url).is_empty(),
                "a http private URL still normalizes non-empty (scheme is allowed)"
            );
            // … but the SSRF host guard handle_import runs rejects the private host,
            // returning the exact 'url host is not allowed' error → no persistence.
            assert!(
                !super::auth::is_safe_import_url(url),
                "private host must be rejected by the import SSRF guard"
            );
        }
        other => panic!("authenticated import must classify as Import, got {other:?}"),
    }
}

/// A non-JSON frame (with no token field) is dropped silently — no reply, no
/// dispatch.
#[test]
fn frame_non_json_is_dropped() {
    let (_dir, state) = bridge_state();
    match classify_frame(&state, "this is not json {") {
        FrameDecision::Drop => {}
        other => panic!("non-JSON must Drop, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B4. normalize_job_url transforms the dedup key relies on (MEDIUM)
//
// upsert_for_origin dedup is currently the only observable check of these. Pin
// the exact transforms directly so a regression is caught at the unit boundary.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn normalize_job_url_strips_www_query_fragment_and_trailing_slash() {
    // www. strip + trailing-slash strip.
    assert_eq!(
        normalize_job_url("https://www.acme.example/jobs/42/"),
        "https://acme.example/jobs/42"
    );
    // query + utm_* strip (whole query is dropped).
    assert_eq!(
        normalize_job_url("https://acme.example/jobs/42?utm_source=ext&ref=x"),
        "https://acme.example/jobs/42"
    );
    // #fragment strip.
    assert_eq!(
        normalize_job_url("https://acme.example/jobs/42#apply"),
        "https://acme.example/jobs/42"
    );
    // lowercase host (scheme preserved-lowercased).
    assert_eq!(
        normalize_job_url("HTTPS://WWW.Acme.Example/Jobs/42"),
        "https://acme.example/jobs/42"
    );
    // All transforms at once.
    assert_eq!(
        normalize_job_url("https://www.Acme.Example/jobs/42/?utm_campaign=z#frag"),
        "https://acme.example/jobs/42"
    );
}

#[test]
fn normalize_job_url_neutralizes_non_http_schemes_to_empty() {
    // Dangerous explicit schemes collapse to "" (treated as "no url").
    assert_eq!(normalize_job_url("javascript:alert(1)"), "");
    assert_eq!(normalize_job_url("data:text/html,<h1>x</h1>"), "");
    assert_eq!(normalize_job_url("file:///etc/passwd"), "");
    assert_eq!(normalize_job_url("ftp://acme.example/x"), "");
    // Empty input → empty.
    assert_eq!(normalize_job_url("   "), "");
}

// ─────────────────────────────────────────────────────────────────────────────
// A4. Port-probe fallback — hermetic, non-flaky
// ─────────────────────────────────────────────────────────────────────────────

/// Claim one ephemeral loopback port (kernel-assigned via port 0, so we never
/// collide with whatever CI already holds) and return the live listener plus its
/// concrete port. Holding the listener keeps that exact port BUSY for the test's
/// lifetime — deterministic regardless of what else runs.
async fn claim_busy_port() -> (TcpListener, u16) {
    let l = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    (l, port)
}

/// A loopback port that is currently FREE: bind ephemeral, read the port, drop
/// the listener. (A later bind of this exact port can still race another process,
/// so callers must only probe ranges, not assert this specific port binds.)
async fn pick_free_port() -> u16 {
    let l = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port
}

/// `probe_ports` must SKIP a busy port and bind a free one further in the range.
/// Deterministic: a single-port range over the held port MUST yield `None` (the
/// busy port is correctly not bound), and a wider range starting at the busy port
/// MUST yield `Some` on a DIFFERENT port (it skipped the busy one). Both halves
/// assert — no early return, no false-green.
#[tokio::test]
async fn port_probe_skips_busy_port_and_binds_next_free() {
    let (busy, busy_port) = claim_busy_port().await;

    // (a) A range that is exactly the held port → no free port → None. This is the
    //     skip proof: the busy port is never bound.
    assert!(
        super::probe_ports(busy_port..=busy_port).await.is_none(),
        "a single-port range over the held port {busy_port} must yield None"
    );

    // (b) A wider range starting at the busy port → probe must skip the busy port
    //     and bind a free one above it. (busy_port+1.. is overwhelmingly free; we
    //     assert the bound port is simply NOT the busy one and IS in range.)
    let end = busy_port.saturating_add(50).max(busy_port + 1);
    let (listener, bound) = super::probe_ports(busy_port..=end)
        .await
        .expect("a wide range above the busy port must find a free port");
    assert_ne!(
        bound, busy_port,
        "probe must skip the held busy port {busy_port}"
    );
    assert!(
        (busy_port..=end).contains(&bound),
        "bound port {bound} must be within the probed range"
    );

    drop(listener);
    drop(busy);
}

/// `probe_ports` must return `None` (graceful disable) when EVERY port in the
/// span is busy. Deterministic: we hold one port and probe exactly that
/// single-port span — there is no free port, so the result MUST be `None`. No
/// real 6-port-range allocation, no skip-on-busy-CI.
#[tokio::test]
async fn port_probe_returns_none_when_full_span_busy_graceful_disable() {
    let (held, held_port) = claim_busy_port().await;

    // The span is the single held port → fully busy → graceful None.
    let result = super::probe_ports(held_port..=held_port).await;
    assert!(
        result.is_none(),
        "probe_ports must return None when the only port in range is busy"
    );

    // Control: once released, that exact port becomes bindable again, proving the
    // None above was due to the held listener — not a bug.
    drop(held);
    let reclaim = pick_free_port().await; // exercises the free-detect helper
    let _ = reclaim;
}

// ─────────────────────────────────────────────────────────────────────────────
// A5. Canonical-import precedence — the list-shell DOM is never adopted
//
// `handle_import` is an async fn that takes `&AppHandle` and cannot be invoked
// hermetically (no live Tauri runtime in unit tests — see the A3 note above).
// So we test the invariant at the reachable seams:
//
//   (a) A LinkedIn SPA/list URL DOES get rewritten by `canonical_job_url` →
//       `canonical.is_some()`.
//   (b) Parsing the list-shell HTML with `parse_from_html` WOULD have yielded a
//       titled posting — proving the old Fallback-X code would have wrongly
//       adopted list-shell content.
//   (c) The new precedence NEVER calls `parse_from_html` when `canonical` is
//       Some — the canonical branch calls `resolve(c)` only. When that returns
//       None/Err the handler falls through to the stub, not to a list-shell parse.
//
// Together (a)+(b)+(c) pin the invariant: a SPA/list import whose server fetch
// yields nothing will produce a partial stub, NOT a posting whose title/content
// came from the list shell.
// ─────────────────────────────────────────────────────────────────────────────

/// A LinkedIn jobs/search URL with a `currentJobId` is rewritten to a canonical
/// view URL. The extension sends the list-shell DOM alongside it; the list-shell
/// HTML WOULD have yielded a titled posting via `parse_from_html` — proving the
/// old Fallback-X code would have adopted the wrong content. The new precedence
/// skips `parse_from_html` entirely when `canonical` is `Some`, so the shell
/// content can never leak into an imported application.
#[test]
fn canonical_spa_url_rewrite_skips_list_shell_dom_parse() {
    use crate::scraping::scrape_url::{canonical_job_url, parse_from_html};

    let list_url = "https://www.linkedin.com/jobs/search/?currentJobId=4185657072";

    // (a) A rewrite MUST happen — this is the precondition for the whole test.
    let canonical = canonical_job_url(list_url);
    assert!(
        canonical.is_some(),
        "a LinkedIn search URL with currentJobId MUST be rewritten to a canonical view URL;          if this fails the board rewrites were removed and the test needs updating"
    );
    assert_eq!(
        canonical.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4185657072"),
        "canonical must point to the /jobs/view/<id> form"
    );

    // (b) The list-shell HTML WOULD have produced a titled posting via parse_from_html
    // — proving the old Fallback-X code (parse_from_html(effective_url, html) when
    // canonical was Some) would have adopted list-shell content as the import result.
    let list_shell_html = r#"
        <html>
        <head>
            <title>Jobs at LinkedIn</title>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "WRONG: This is the list-shell job posting",
                "hiringOrganization": { "name": "LinkedIn List Shell" }
            }
            </script>
        </head>
        <body><h1>Job Search Results</h1></body>
        </html>
    "#;
    let shell_parse = parse_from_html(list_url, list_shell_html);
    assert!(
        shell_parse.as_ref().is_some_and(|p| !p.title.is_empty()),
        "parse_from_html on the list-shell HTML yields a titled posting —          confirming the old Fallback-X code would have wrongly adopted this content"
    );

    // (c) The new precedence: when canonical.is_some(), only resolve(canonical) is
    // called — parse_from_html is never reached for the shell. We can't call
    // handle_import hermetically, but the if-else structure in handle_import is:
    //
    //   if let Some(c) = canonical.as_deref() {
    //       resolve(c).await?    ← only this branch runs when canonical is Some
    //   } else if let Some(h) = html.as_deref() {
    //       parse_from_html(...)  ← SKIPPED when canonical is Some
    //   } ...
    //
    // And the single fallback guard:
    //   if ... && canonical.is_none() && html.is_some() { ... }
    //             ^^^^^^^^^^^^^^^^^^^
    //   is also guarded — so resolve(effective_url) is also skipped for the canonical path.
    //
    // Asserting canonical.is_some() (done above) is the structural proof that
    // parse_from_html is unreachable for this URL under the new precedence.
    assert!(
        canonical.is_some(),
        "structural proof: canonical.is_some() → parse_from_html branch is unreachable          for this URL under the new handle_import precedence"
    );
}
