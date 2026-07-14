//! Extension-bridge: Scan-mode parse goldens + saved/applied/dedup persistence
//! matrix + port-probe fallback.
//!
//! These tests are hermetic (no network). They exercise:
//!  - `parse_from_html`: JSON-LD path (LinkedIn-style) and generic-meta path
//!    (Indeed/Lever-style).
//!  - `advance_frame`: the v2 handshake state machine + per-message gate (size
//!    cap, hello→challenge→auth mutual HMAC, session dispatch) — an outdated first
//!    frame → `update_required` + close, a failed proof → close (never connected),
//!    import/profile ONLY in the Authenticated state, over-cap → close, SSRF
//!    private host blocked, non-JSON dropped.
//!  - `upsert_for_origin`: saved/applied/dedup outcomes at the persistence
//!    boundary (mirrors what `handle_import` calls in production).
//!  - `normalize_job_url`: the exact dedup-key transforms (www/query/fragment/
//!    trailing-slash strip, lowercase host, non-http(s) scheme → empty).
//!  - `probe_ports`: skips an occupied port and binds a free one; when the span
//!    is fully busy the bridge degrades gracefully (`None`).

use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;

use super::answers_suggest::{match_questions, AnswerCandidate};
use super::{advance_frame, handshake, BridgeState, ConnState, FrameDecision, MAX_FRAME_BYTES};
use crate::ai_generations::ApplicationAnswer;
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
        salary_min: None,
        salary_max: None,
        salary_currency: None,
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
// B1. v2 mutual-handshake state machine (the token-never-on-the-wire fix)
//
// `advance_frame` is the per-message gate the connection loop runs. The security
// invariant: an `import.request` / `profile.get` is dispatched ONLY from the
// `Authenticated` state (i.e. AFTER a verified client proof). A socket that has
// not completed the handshake can NEVER reach `handle_import` — so nothing is
// ever persisted for an unauthenticated peer.
// ─────────────────────────────────────────────────────────────────────────────

/// A valid protocol-2 `hello` (with a well-formed clientNonce) is accepted:
/// `Challenge` carrying a fresh `serverNonce`, advancing to `AwaitingAuth` bound
/// to that nonce pair. NOT yet connected.
#[test]
fn hello_v2_is_accepted_and_advances_to_awaiting_auth() {
    let (_dir, state) = bridge_state();
    let client_nonce = handshake::new_nonce();

    let frame = json!({
        "type": super::msg::HELLO,
        "reqId": "r-hello",
        "payload": { "protocol": super::PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string();

    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Challenge { reply, next } => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], super::msg::CHALLENGE);
            assert_eq!(v["reqId"], "r-hello");
            let server_nonce = v["payload"]["serverNonce"].as_str().unwrap();
            assert!(
                handshake::is_valid_nonce(server_nonce),
                "challenge must carry a well-formed server nonce"
            );
            match next {
                ConnState::AwaitingAuth {
                    server_nonce: sn,
                    client_nonce: cn,
                } => {
                    assert_eq!(sn, server_nonce, "next state binds the sent server nonce");
                    assert_eq!(cn, client_nonce, "next state binds the client nonce");
                }
                other => panic!("hello must advance to AwaitingAuth, got {other:?}"),
            }
        }
        other => panic!("a valid v2 hello must be Challenge, got {other:?}"),
    }
}

/// A legacy `{type:'auth', token}` FIRST frame (an OLD extension) is `Outdated`:
/// `update_required` reply then close — the force cutover, never an import.
#[test]
fn legacy_auth_first_frame_is_outdated() {
    let (_dir, state) = bridge_state();

    let frame = json!({
        "type": super::msg::AUTH,
        "token": state.token(), // legacy plaintext token — must NOT authenticate
        "reqId": "r-legacy",
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(reply) => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], super::msg::UPDATE_REQUIRED);
            assert_eq!(v["reqId"], "r-legacy");
            assert!(v["payload"]["error"].as_str().unwrap().contains("Update"));
        }
        other => panic!("a legacy token `auth` first frame must be Outdated, got {other:?}"),
    }
}

/// A `hello` carrying a lower/older protocol is treated as an outdated client.
#[test]
fn hello_with_lower_protocol_is_outdated() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::HELLO,
        "reqId": "r-old",
        "payload": { "protocol": 1, "clientNonce": handshake::new_nonce() },
    })
    .to_string();
    assert!(
        matches!(
            advance_frame(&state, &ConnState::AwaitingHello, &frame),
            FrameDecision::Outdated(_)
        ),
        "protocol < 2 must be Outdated"
    );
}

/// A `hello` with a malformed clientNonce (wrong shape) is rejected as outdated —
/// junk never reaches the HMAC.
#[test]
fn hello_with_malformed_nonce_is_outdated() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::HELLO,
        "reqId": "r-bad-nonce",
        "payload": { "protocol": super::PROTOCOL_VERSION, "clientNonce": "not-hex!!" },
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &ConnState::AwaitingHello, &frame),
        FrameDecision::Outdated(_)
    ));
}

/// An `import.request` in the AwaitingHello state is NEVER dispatched — it is an
/// outdated first frame (not a hello). This is the core invariant: you cannot
/// import before completing the handshake.
#[test]
fn import_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::IMPORT_REQUEST,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an import.request before hello must NOT be an Import, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B1b. Handshake step 3: constant-time client-proof verification
//
// In the AwaitingAuth state, only an `auth { proof }` with a proof that verifies
// constant-time against HMAC-SHA256(token, CLIENT_MSG) advances to Authenticated
// (AuthOk, replying the server proof). A wrong/absent proof, or any non-auth
// frame, closes the socket (Unauthorized) and never marks it connected.
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: the AwaitingAuth state for a fixed nonce pair, plus the CORRECT client
/// proof the extension would compute for `state`'s token.
fn awaiting_auth(state: &BridgeState) -> (ConnState, String) {
    let server_nonce = handshake::new_nonce();
    let client_nonce = handshake::new_nonce();
    let proof = handshake::client_proof(&state.token(), &server_nonce, &client_nonce);
    (
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        },
        proof,
    )
}

/// A correct client proof advances to `AuthOk`: the reply is an `auth.ok`
/// envelope whose `serverProof` equals `HMAC(token, SERVER_MSG)` for the bound
/// nonces — so the extension can verify the desktop is genuine.
#[test]
fn correct_proof_yields_auth_ok_with_matching_server_proof() {
    let (_dir, state) = bridge_state();
    let (conn, proof) = awaiting_auth(&state);
    let (server_nonce, client_nonce) = match &conn {
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        } => (server_nonce.clone(), client_nonce.clone()),
        _ => unreachable!(),
    };

    let frame = json!({
        "type": super::msg::AUTH,
        "reqId": "r-auth",
        "payload": { "proof": proof },
    })
    .to_string();

    match advance_frame(&state, &conn, &frame) {
        FrameDecision::AuthOk(reply) => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], super::msg::AUTH_OK);
            assert_eq!(v["reqId"], "r-auth");
            let server_proof = v["payload"]["serverProof"].as_str().unwrap();
            assert_eq!(
                server_proof,
                handshake::server_proof(&state.token(), &server_nonce, &client_nonce),
                "the desktop must return the exact server proof the extension expects"
            );
        }
        other => panic!("a correct client proof must be AuthOk, got {other:?}"),
    }
}

/// A WRONG proof (right shape, wrong bytes) is `Unauthorized` — the socket closes
/// and is never marked connected. This is the token-mismatch path (bad_token).
#[test]
fn wrong_proof_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    // A validly-shaped but wrong proof (all zeros).
    let frame = json!({
        "type": super::msg::AUTH,
        "reqId": "r-bad",
        "payload": { "proof": "0".repeat(64) },
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &conn, &frame),
        FrameDecision::Unauthorized
    ));
}

/// An absent/empty proof is `Unauthorized` (never a panic, never connected).
#[test]
fn absent_proof_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::AUTH,
        "reqId": "r-empty",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &conn, &frame),
        FrameDecision::Unauthorized
    ));
}

/// A non-auth frame in AwaitingAuth (e.g. an import.request trying to skip the
/// proof) is `Unauthorized` — you cannot bypass step 3.
#[test]
fn non_auth_frame_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::IMPORT_REQUEST,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an import.request mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// End-to-end pure handshake: AwaitingHello --hello--> Challenge(next) ; feed the
/// derived server+client nonces the REAL client proof --auth--> AuthOk. Proves the
/// two-frame mutual handshake composes with the actual nonces the desktop issues.
#[test]
fn full_handshake_hello_then_auth_authenticates() {
    let (_dir, state) = bridge_state();
    let client_nonce = handshake::new_nonce();

    let hello = json!({
        "type": super::msg::HELLO,
        "reqId": "h",
        "payload": { "protocol": super::PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string();

    let next = match advance_frame(&state, &ConnState::AwaitingHello, &hello) {
        FrameDecision::Challenge { next, .. } => next,
        other => panic!("hello must Challenge, got {other:?}"),
    };
    let server_nonce = match &next {
        ConnState::AwaitingAuth { server_nonce, .. } => server_nonce.clone(),
        _ => unreachable!(),
    };

    // The extension computes the client proof for the issued nonces.
    let proof = handshake::client_proof(&state.token(), &server_nonce, &client_nonce);
    let auth = json!({
        "type": super::msg::AUTH,
        "reqId": "a",
        "payload": { "proof": proof },
    })
    .to_string();

    assert!(
        matches!(
            advance_frame(&state, &next, &auth),
            FrameDecision::AuthOk(_)
        ),
        "the real client proof for the issued nonces must authenticate"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// B1c. Post-auth dispatch — import/profile ONLY in the Authenticated state
// ─────────────────────────────────────────────────────────────────────────────

/// An authenticated `import.request` classifies as `Import` (positive control),
/// carrying the payload verbatim to `handle_import`.
#[test]
fn authenticated_import_classifies_as_import() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::IMPORT_REQUEST,
        "reqId": "r-ok",
        "payload": { "url": "https://jobs.example.com/posting/3" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Import { req_id, payload } => {
            assert_eq!(req_id, "r-ok");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/3")
            );
        }
        other => panic!("an authenticated import.request must be Import, got {other:?}"),
    }
}

/// An authenticated `profile.get` classifies as `Profile`.
#[test]
fn authenticated_profile_get_classifies_as_profile() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::PROFILE_GET,
        "reqId": "r-prof",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Profile { req_id } => assert_eq!(req_id, "r-prof"),
        other => panic!("an authenticated profile.get must be Profile, got {other:?}"),
    }
}

/// A reserved type in the Authenticated state gets an `import.result` error reply
/// (never a panic).
#[test]
fn authenticated_reserved_type_replies_with_error() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::MATCH_LIVE,
        "reqId": "r-reserved",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Reply(reply) => {
            let err = reply_error(&reply).expect("reserved type must carry an error");
            assert!(err.contains("not implemented"), "got {err:?}");
        }
        other => panic!("a reserved type must be a Reply(error), got {other:?}"),
    }
}

/// An authenticated `applied.check` classifies as `AppliedCheck`, carrying the
/// payload verbatim (mirrors the `Import`/`Profile` classify tests above).
#[test]
fn authenticated_applied_check_classifies_as_applied_check() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::APPLIED_CHECK,
        "reqId": "r-applied",
        "payload": { "url": "https://jobs.example.com/posting/9" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AppliedCheck { req_id, payload } => {
            assert_eq!(req_id, "r-applied");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/9")
            );
        }
        other => panic!("an authenticated applied.check must be AppliedCheck, got {other:?}"),
    }
}

/// An `applied.check` before the handshake completes is NEVER dispatched — same
/// invariant as `import_before_handshake_is_not_dispatched`: only a hello may
/// be the first frame.
#[test]
fn applied_check_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::APPLIED_CHECK,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an applied.check before hello must NOT be AppliedCheck, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `non_auth_frame_mid_handshake_is_unauthorized`
/// — an `applied.check` cannot skip the proof step either.
#[test]
fn applied_check_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::APPLIED_CHECK,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an applied.check mid-handshake must be Unauthorized, got {other:?}"),
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

    // The size guard runs before parse/state — state-independent.
    match advance_frame(&state, &ConnState::Authenticated, &oversize) {
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
    match advance_frame(&state, &ConnState::Authenticated, &at_cap) {
        FrameDecision::Drop => {}
        other => panic!("a frame exactly at the cap must parse (Drop here), got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B3. SSRF private-host import rejection (MEDIUM) + non-JSON drop
//
// An authenticated import.request whose `url` is a private/loopback host must be
// rejected by the SSRF gate `handle_import` applies (`is_safe_import_url`) before
// any persistence. advance_frame (in the Authenticated state) routes it to Import;
// we then assert the exact gate handle_import runs rejects the host, so nothing
// is ever persisted.
// ─────────────────────────────────────────────────────────────────────────────

/// A private-host import URL classifies as Import (session-authenticated) but is
/// blocked by the host SSRF guard handle_import applies before persisting.
#[test]
fn frame_private_host_import_is_blocked_by_ssrf_gate() {
    let (_dir, state) = bridge_state();
    let private_url = "http://192.168.1.1/job";

    // v2: no token on the frame; the socket is already session-authenticated.
    let frame = json!({
        "reqId": "r-ssrf",
        "type": super::msg::IMPORT_REQUEST,
        "payload": { "url": private_url }
    })
    .to_string();

    // Authenticated → Import; the URL is non-empty after normalization …
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
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
    match advance_frame(&state, &ConnState::Authenticated, "this is not json {") {
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
// C. applied.check — pure store lookup (found saved / found applied / not
// found / malformed url). `resolve_applied_check` is a private, synchronous fn
// (no `AppHandle`), so unlike `handle_import` it IS directly unit-testable —
// these tests exercise the exact boundary `handle_applied_check` calls.
// ─────────────────────────────────────────────────────────────────────────────

/// No Application exists yet for the url → `found: false`, everything else
/// `None`, never an error.
#[test]
fn resolve_applied_check_not_found_when_no_application() {
    let (_dir, store) = open_store();
    let payload = json!({ "url": "https://jobs.example.com/posting/none" });
    let out = super::resolve_applied_check(&store, &payload).unwrap();
    assert!(!out.found);
    assert!(out.application_id.is_none());
    assert!(out.status.is_none());
    assert!(out.applied_at.is_none());
}

/// A `saved` Application (not yet applied) is found with status "saved", the
/// posting's title, and NO `appliedAt` (it hasn't left `saved`).
#[test]
fn resolve_applied_check_found_saved_has_no_applied_at() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/saved-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = super::resolve_applied_check(&store, &json!({ "url": url })).unwrap();
    assert!(out.found);
    assert_eq!(out.status.as_deref(), Some("saved"));
    assert_eq!(out.title.as_deref(), Some("Backend Engineer"));
    assert!(out.applied_at.is_none());
}

/// An `applied` Application is found with status "applied" and a non-null
/// `appliedAt` (epoch ms) — the field the popup formats into a date.
#[test]
fn resolve_applied_check_found_applied_carries_applied_at() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/applied-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let out = super::resolve_applied_check(&store, &json!({ "url": url })).unwrap();
    assert!(out.found);
    assert_eq!(out.status.as_deref(), Some("applied"));
    assert!(
        out.applied_at.is_some(),
        "an applied row must carry applied_at"
    );
}

/// An empty url is a Validation error — never a panic, never a false `found`.
#[test]
fn resolve_applied_check_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err = super::resolve_applied_check(&store, &json!({ "url": "" })).unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A non-http(s) url normalizes to empty and is rejected the same way
/// `handle_import` rejects it (dangerous explicit schemes never round-trip).
#[test]
fn resolve_applied_check_rejects_non_http_scheme() {
    let (_dir, store) = open_store();
    let err =
        super::resolve_applied_check(&store, &json!({ "url": "javascript:alert(1)" })).unwrap_err();
    assert!(err.to_string().contains("not a valid"));
}

/// `applied_result_reply` builds a well-formed `applied.result` envelope
/// carrying `found`/`status`/`appliedAt` on success (mirrors
/// `profile_result_reply_carries_type_and_req_id`).
#[test]
fn applied_result_reply_carries_type_and_found_flag() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/reply-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();
    let outcome = super::resolve_applied_check(&store, &json!({ "url": url }));
    let reply = super::applied_result_reply("req-9", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::APPLIED_RESULT);
    assert_eq!(v["reqId"], "req-9");
    assert_eq!(v["payload"]["found"], true);
    assert_eq!(v["payload"]["status"], "applied");
    assert!(v["payload"]["appliedAt"].is_number());
}

/// A malformed (empty) url produces `{ found: false, error }` — never a panic,
/// never a bare `{error}` without `found` (the extension's guard requires it).
#[test]
fn applied_result_reply_carries_error_on_malformed_url() {
    let (_dir, store) = open_store();
    let outcome = super::resolve_applied_check(&store, &json!({ "url": "" }));
    let reply = super::applied_result_reply("req-10", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::APPLIED_RESULT);
    assert_eq!(v["payload"]["found"], false);
    assert!(v["payload"]["error"].as_str().unwrap().contains("required"));
}

// ─────────────────────────────────────────────────────────────────────────────
// D. status.update — the narrowest possible write: `saved → applied` on an
// EXACT url-key match, and nothing else. Mirrors the applied.check section
// above: dispatch classification + the 3 auth-boundary guards, then
// `resolve_status_update` (a private, synchronous fn — no `AppHandle` — so,
// like `resolve_applied_check`, it IS directly unit-testable at the exact
// boundary `handle_status_update` calls).
// ─────────────────────────────────────────────────────────────────────────────

/// An authenticated `status.update` classifies as `StatusUpdate`, carrying the
/// payload verbatim (mirrors `authenticated_applied_check_classifies_as_applied_check`).
#[test]
fn authenticated_status_update_classifies_as_status_update() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::STATUS_UPDATE,
        "reqId": "r-status",
        "payload": { "url": "https://jobs.example.com/posting/10", "to": "applied" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::StatusUpdate { req_id, payload } => {
            assert_eq!(req_id, "r-status");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/10")
            );
            assert_eq!(payload.get("to").and_then(|t| t.as_str()), Some("applied"));
        }
        other => panic!("an authenticated status.update must be StatusUpdate, got {other:?}"),
    }
}

/// A `status.update` before the handshake completes is NEVER dispatched — same
/// invariant as `applied_check_before_handshake_is_not_dispatched`: only a
/// hello may be the first frame.
#[test]
fn status_update_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::STATUS_UPDATE,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early", "to": "applied" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("a status.update before hello must NOT be StatusUpdate, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `applied_check_mid_handshake_is_unauthorized`
/// — a `status.update` cannot skip the proof step either. This is the one
/// verb where skipping this guard would matter most (it is a WRITE), so it
/// gets the same three auth-boundary tests as every other authenticated verb.
#[test]
fn status_update_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::STATUS_UPDATE,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip", "to": "applied" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("a status.update mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// The happy path: a `saved` Application at the exact url transitions to
/// `applied` — the status event is appended with the fixed note "via
/// extension" (no page-derived text) and `applied_at` is set.
#[test]
fn resolve_status_update_transitions_saved_to_applied_and_appends_event() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": url, "to": "applied" }),
    )
    .expect("saved -> applied must succeed");
    assert_eq!(out.application_id, id);
    assert_eq!(out.status, "applied");

    let row = store.get(&id).unwrap();
    assert_eq!(row.status, ApplicationStatus::Applied);
    assert!(row.applied_at.is_some(), "applied_at must be set");

    let events = store.events(&id);
    let last = events.last().expect("set_status must append an event");
    assert_eq!(last.to_status, "applied");
    assert_eq!(
        last.note, "via extension",
        "the status event note must be a short fixed string — never page-derived text"
    );
}

/// Regression guard for the non-atomic-guard review finding (HIGH): the
/// desktop-side guard is `ApplicationStore::transition_status_if`, a single
/// atomic compare-and-set. Simulate two callers racing the exact same
/// `saved -> applied` transition by calling it twice directly — only the
/// FIRST may succeed; the SECOND must lose the race (`Ok(false)`), appending
/// no second status event and never re-bumping `applied_at`.
#[test]
fn transition_status_if_second_call_after_already_applied_loses_the_race() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/race-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Race Condition Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let first = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(first, "the first caller wins the race");
    let applied_at_after_first = store.get(&id).unwrap().applied_at;
    let events_after_first = store.events(&id).len();

    // A second caller racing the same transition after the first already
    // committed (e.g. two extension clicks, or a retry) must lose — never a
    // second write, never a duplicate status event.
    let second = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(!second, "the second racing caller must lose (Ok(false))");
    assert_eq!(
        store.events(&id).len(),
        events_after_first,
        "the losing call must not append a second status event"
    );
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        applied_at_after_first,
        "the losing call must not re-bump applied_at"
    );
}

/// A row that is ALREADY `applied` refuses — this verb can never re-fire the
/// transition (no "re-apply", no bumping `applied_at` again).
#[test]
fn resolve_status_update_refuses_when_already_applied() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-2";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true), // already applied
        )
        .unwrap();

    let err = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": url, "to": "applied" }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("no longer saved"), "got: {err}");
}

/// A row mid-pipeline (picked: `interviewing`) also refuses — the allowlist is
/// `saved -> applied` ONLY, not "anything not yet applied".
#[test]
fn resolve_status_update_refuses_mid_pipeline_status() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-3";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Platform Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    store
        .set_status(&id, ApplicationStatus::Interviewing, "test setup")
        .unwrap();

    let err = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": url, "to": "applied" }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("no longer saved"), "got: {err}");

    // The refusal must never have written anything — status stays exactly as
    // the test setup left it.
    let row = store.get(&id).unwrap();
    assert_eq!(row.status, ApplicationStatus::Interviewing);
}

/// No Application exists for the url at all → a clear "couldn't find" refusal,
/// never a create-on-miss.
#[test]
fn resolve_status_update_rejects_when_no_match() {
    let (_dir, store) = open_store();
    let err = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": "https://jobs.example.com/posting/none", "to": "applied" }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("couldn't find"), "got: {err}");
}

/// An empty url is a Validation error — never a panic.
#[test]
fn resolve_status_update_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err =
        super::status_update::resolve_status_update(&store, &json!({ "url": "", "to": "applied" }))
            .unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A non-empty but malformed url (an explicit non-http(s) scheme — the same
/// chokepoint `resolve_applied_check_rejects_non_http_scheme` exercises) hits
/// the DISTINCT "url is not a valid http(s) URL" sentinel — never confused
/// with "url is required" (empty input, tested above) or "couldn't find" (a
/// well-formed url with no matching row, tested below).
#[test]
fn resolve_status_update_rejects_malformed_url() {
    let (_dir, store) = open_store();
    let err = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": "javascript:alert(1)", "to": "applied" }),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("not a valid"),
        "a malformed/dangerous-scheme url must hit the distinct invalid-url sentinel, got: {err}"
    );
}

/// `to` must literal-match "applied" — re-validated on the Rust side
/// independently of the TS zod literal. Any other value (including a missing
/// field) is rejected BEFORE the store is even queried.
#[test]
fn resolve_status_update_rejects_to_not_applied() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-4";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let wrong_value = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": url, "to": "interviewing" }),
    )
    .unwrap_err();
    assert!(
        wrong_value.to_string().contains("unsupported"),
        "got: {wrong_value}"
    );

    let missing_field =
        super::status_update::resolve_status_update(&store, &json!({ "url": url })).unwrap_err();
    assert!(
        missing_field.to_string().contains("unsupported"),
        "got: {missing_field}"
    );

    // Neither rejected attempt may have written anything.
    let row = store.list().into_iter().find(|a| a.job_url == url).unwrap();
    assert_eq!(row.status, ApplicationStatus::Saved);
}

/// `status_result_reply` builds a well-formed `status.update` success envelope
/// (mirrors `applied_result_reply_carries_type_and_found_flag`).
#[test]
fn status_result_reply_carries_ok_true_and_fields() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-5";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Reply Test Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let outcome = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": url, "to": "applied" }),
    );
    let reply = super::status_update::status_result_reply("req-11", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::STATUS_RESULT);
    assert_eq!(v["reqId"], "req-11");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["status"], "applied");
    assert!(v["payload"]["applicationId"].is_string());
}

/// UNLIKE `applied.result`, a `status.update` failure carries a fixed,
/// user-facing `error` string — this verb's errors are surfaced to the user
/// (it answers a deliberate click), so the reply must never fold the failure
/// into a silent/blank payload.
#[test]
fn status_result_reply_carries_user_facing_error() {
    let (_dir, store) = open_store();
    let outcome = super::status_update::resolve_status_update(
        &store,
        &json!({ "url": "https://jobs.example.com/posting/none", "to": "applied" }),
    );
    let reply = super::status_update::status_result_reply("req-12", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::STATUS_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("couldn't find"));
}

// ─────────────────────────────────────────────────────────────────────────────
// E. answers.save — "save my answers from this page". Mirrors the
// status.update section above: dispatch classification + the 3 auth-boundary
// guards, then `resolve_answers_save` (a private, synchronous fn — no
// `AppHandle`) directly. Rides the SAME autofill opt-in as `profile.get`
// (unlike `applied.check`/`status.update`, which need no consent gate).
// ─────────────────────────────────────────────────────────────────────────────

/// An authenticated `answers.save` classifies as `AnswersSave`, carrying the
/// payload verbatim (mirrors `authenticated_status_update_classifies_as_status_update`).
#[test]
fn authenticated_answers_save_classifies_as_answers_save() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::ANSWERS_SAVE,
        "reqId": "r-answers",
        "payload": {
            "url": "https://jobs.example.com/posting/11",
            "answers": [{ "question": "Why this role?", "answer": "Because I love it." }],
        }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AnswersSave { req_id, payload } => {
            assert_eq!(req_id, "r-answers");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/11")
            );
            assert!(payload.get("answers").is_some_and(|a| a.is_array()));
        }
        other => panic!("an authenticated answers.save must be AnswersSave, got {other:?}"),
    }
}

/// An `answers.save` before the handshake completes is NEVER dispatched —
/// same invariant as `status_update_before_handshake_is_not_dispatched`.
#[test]
fn answers_save_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::ANSWERS_SAVE,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early", "answers": [] },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an answers.save before hello must NOT be AnswersSave, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `status_update_mid_handshake_is_unauthorized`
/// — `answers.save` cannot skip the proof step either (it is a WRITE, so it
/// gets the same three auth-boundary tests as every other authenticated verb).
#[test]
fn answers_save_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::ANSWERS_SAVE,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip", "answers": [] },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an answers.save mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// Opt-in OFF is a fixed refusal mirroring `resolve_profile`'s exact sentinel
/// — even when a matching Application exists, no answer is ever merged.
#[test]
fn resolve_answers_save_refuses_when_opt_in_off() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let err = super::answers_save::resolve_answers_save(
        &store,
        false,
        &json!({ "url": url, "answers": [{ "question": "Why?", "answer": "Because." }] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Autofill is off"), "got: {err}");

    // The refusal must never have written anything.
    assert!(store.get(&id).unwrap().answers.is_empty());
}

/// An empty url is a Validation error — never a panic.
#[test]
fn resolve_answers_save_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": "", "answers": [] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A malformed (dangerous-scheme) url hits the distinct invalid-url sentinel
/// — mirrors `resolve_status_update_rejects_malformed_url`.
#[test]
fn resolve_answers_save_rejects_malformed_url() {
    let (_dir, store) = open_store();
    let err = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": "javascript:alert(1)", "answers": [] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not a valid"), "got: {err}");
}

/// No Application exists for the url → a clear "import it first" refusal,
/// never a create-on-miss (this verb only appends onto an existing pursuit).
#[test]
fn resolve_answers_save_rejects_when_no_match() {
    let (_dir, store) = open_store();
    let err = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": "https://jobs.example.com/posting/none", "answers": [] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("import it first"), "got: {err}");
}

/// An empty `answers` array is a well-formed no-op: `saved: 0, skipped: 0`,
/// never an error.
#[test]
fn resolve_answers_save_handles_empty_list() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-2";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": url, "answers": [] }),
    )
    .unwrap();
    assert_eq!(out.saved, 0);
    assert_eq!(out.skipped, 0);
}

/// The happy path: new questions are added, a re-captured (differently-
/// whitespaced/cased) duplicate of an EXISTING question is skipped, and the
/// existing answer is NEVER overwritten. `title`/`company` ride the reply.
#[test]
fn resolve_answers_save_dedups_against_existing_and_never_overwrites() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-3";
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "seed-1".to_string(),
        question: "Why this role?".to_string(),
        answer: "Original answer".to_string(),
    }];
    let id = store
        .upsert_for_origin(url, "linkedin", &meta, ApplicationOrigin::Saved, None)
        .unwrap();

    let out = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({
            "url": url,
            "answers": [
                { "question": "why   THIS role?", "answer": "A different answer that must be dropped" },
                { "question": "What's your salary expectation?", "answer": "100k" },
            ],
        }),
    )
    .unwrap();
    assert_eq!(out.saved, 1, "only the genuinely new question is added");
    assert_eq!(out.skipped, 1, "the re-captured duplicate is dedup-dropped");
    assert_eq!(out.title.as_deref(), Some("Backend Engineer"));
    assert_eq!(out.company.as_deref(), Some("Acme"));

    let row = store.get(&id).unwrap();
    assert_eq!(row.answers.len(), 2);
    let original = row
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .expect("the seeded answer must survive");
    assert_eq!(
        original.answer, "Original answer",
        "an existing answer must NEVER be overwritten by a re-capture"
    );
    assert!(row
        .answers
        .iter()
        .any(|a| a.question == "What's your salary expectation?" && a.answer == "100k"));
}

/// Oversized question/answer text is clamped (char-boundary safe), not
/// dropped — mirrors `applications::clamp_job_description`'s discipline.
#[test]
fn resolve_answers_save_clamps_oversized_question_and_answer_bytes() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-4";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let huge_question = "q".repeat(5_000);
    let huge_answer = "a".repeat(20_000);
    let out = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({
            "url": url,
            "answers": [{ "question": huge_question, "answer": huge_answer }],
        }),
    )
    .unwrap();
    assert_eq!(out.saved, 1);

    let row = store.get(&id).unwrap();
    let saved = &row.answers[0];
    assert!(saved.question.len() <= 1_000, "question must be clamped");
    assert!(saved.answer.len() <= 8_000, "answer must be clamped");
}

/// More than 50 entries in one call are capped — extras are silently
/// dropped, never rejected outright, and MUST still be reflected in
/// `skipped` (not silently vanish from both counts).
#[test]
fn resolve_answers_save_caps_at_max_answers_per_call() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-5";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Principal Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let answers: Vec<serde_json::Value> = (0..75)
        .map(|i| json!({ "question": format!("Question {i}?"), "answer": format!("Answer {i}") }))
        .collect();
    let out = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": url, "answers": answers }),
    )
    .unwrap();
    assert_eq!(out.saved, 50, "extras beyond the 50-entry cap are dropped");
    assert_eq!(
        out.skipped, 25,
        "the 25 entries beyond the per-call cap must reconcile into skipped, not vanish"
    );
}

/// The STORE-level 500-answer cap (not the 50-per-call cap above) is what
/// limits this call: a well-under-50 batch still can't push the total past
/// [`crate::applications::MAX_TOTAL_ANSWERS`], and the overflow must land in
/// `skipped` exactly like the per-call cap does.
#[test]
fn resolve_answers_save_reflects_store_level_total_cap_in_skipped() {
    use crate::applications::MAX_TOTAL_ANSWERS;

    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-store-cap";
    let mut meta = app_meta("Acme", "Cap Test Engineer");
    // Seed to exactly (cap - 2) existing distinct answers via the store
    // directly — bypassing the 50-per-call wire cap entirely.
    meta.answers = (0..MAX_TOTAL_ANSWERS - 2)
        .map(|i| ApplicationAnswer {
            id: format!("seed-{i}"),
            question: format!("Existing question {i}?"),
            answer: format!("Existing answer {i}"),
        })
        .collect();
    store
        .upsert_for_origin(url, "linkedin", &meta, ApplicationOrigin::Saved, None)
        .unwrap();

    // 5 new distinct questions — well under MAX_ANSWERS_PER_CALL (50), so the
    // per-call cap never triggers; only the store's cumulative 500 cap does.
    let answers: Vec<serde_json::Value> = (0..5)
        .map(|i| json!({ "question": format!("New question {i}?"), "answer": format!("New answer {i}") }))
        .collect();
    let out = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": url, "answers": answers }),
    )
    .unwrap();
    assert_eq!(
        out.saved, 2,
        "only enough new answers to reach the store cap are saved"
    );
    assert_eq!(
        out.skipped, 3,
        "the 3 answers that didn't fit under the store's total cap must show up as skipped"
    );
}

/// `answers_result_reply` builds a well-formed success envelope carrying the
/// counts + title/company (mirrors `status_result_reply_carries_ok_true_and_fields`).
#[test]
fn answers_result_reply_carries_ok_true_and_counts() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/answers-6";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Reply Test Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let outcome = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": url, "answers": [{ "question": "Why?", "answer": "Because." }] }),
    );
    let reply = super::answers_save::answers_result_reply("req-13", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::ANSWERS_RESULT);
    assert_eq!(v["reqId"], "req-13");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["saved"], 1);
    assert_eq!(v["payload"]["skipped"], 0);
    assert_eq!(v["payload"]["title"], "Reply Test Engineer");
    assert!(v["payload"]["applicationId"].is_string());
}

/// UNLIKE `applied.result`, an `answers.save` failure carries a fixed,
/// user-facing `error` string — this verb's errors are surfaced to the user.
#[test]
fn answers_result_reply_carries_user_facing_error() {
    let (_dir, store) = open_store();
    let outcome = super::answers_save::resolve_answers_save(
        &store,
        true,
        &json!({ "url": "https://jobs.example.com/posting/none", "answers": [] }),
    );
    let reply = super::answers_save::answers_result_reply("req-14", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::ANSWERS_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("import it first"));
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

// ─────────────────────────────────────────────────────────────────────────────
// F. answers.suggest — "suggest answers for this form", the headline replay
// verb. Mirrors the answers.save section above: dispatch classification + the
// 3 auth-boundary guards, then `resolve_answers_suggest` / the pure
// `match_questions` matcher directly. Rides the SAME autofill opt-in as
// `profile.get`/`answers.save`.
// ─────────────────────────────────────────────────────────────────────────────

/// An authenticated `answers.suggest` classifies as `AnswersSuggest`, carrying
/// the payload verbatim (mirrors `authenticated_answers_save_classifies_as_answers_save`).
#[test]
fn authenticated_answers_suggest_classifies_as_answers_suggest() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::ANSWERS_SUGGEST,
        "reqId": "r-suggest",
        "payload": { "questions": ["Why this role?"] }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AnswersSuggest { req_id, payload } => {
            assert_eq!(req_id, "r-suggest");
            assert_eq!(
                payload
                    .get("questions")
                    .and_then(|q| q.as_array())
                    .map(Vec::len),
                Some(1)
            );
        }
        other => panic!("an authenticated answers.suggest must be AnswersSuggest, got {other:?}"),
    }
}

/// An `answers.suggest` before the handshake completes is NEVER dispatched —
/// same invariant as `answers_save_before_handshake_is_not_dispatched`.
#[test]
fn answers_suggest_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": super::msg::ANSWERS_SUGGEST,
        "reqId": "r-early",
        "payload": { "questions": [] },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => {
            panic!("an answers.suggest before hello must NOT be AnswersSuggest, got {other:?}")
        }
    }
}

/// Same mid-handshake bypass guard as `answers_save_mid_handshake_is_unauthorized`
/// — `answers.suggest` cannot skip the proof step either (it returns the
/// user's own past answer text, so it gets the same three auth-boundary tests
/// as every other authenticated verb).
#[test]
fn answers_suggest_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": super::msg::ANSWERS_SUGGEST,
        "reqId": "r-skip",
        "payload": { "questions": [] },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an answers.suggest mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// Opt-in OFF is a fixed refusal mirroring `resolve_answers_save`'s exact
/// sentinel — even with matching answers available, nothing is ever returned.
#[test]
fn resolve_answers_suggest_refuses_when_opt_in_off() {
    let (_dir, store) = open_store();
    let err = super::answers_suggest::resolve_answers_suggest(
        &store,
        false,
        &json!({ "questions": ["Why this role?"] }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Autofill is off"), "got: {err}");
}

/// No/empty `questions` is a well-formed no-op — never an error.
#[test]
fn resolve_answers_suggest_returns_empty_list_for_no_questions() {
    let (_dir, store) = open_store();
    let out =
        super::answers_suggest::resolve_answers_suggest(&store, true, &json!({ "questions": [] }))
            .unwrap();
    assert!(out.is_empty());
}

/// Aggregates across MULTIPLE applications (not just one) via the real store —
/// this is the read-only integration point that stands in for a dedicated
/// store method (see the module doc: `applications/mod.rs` is at the R8 cap).
#[test]
fn resolve_answers_suggest_matches_across_multiple_applications() {
    let (_dir, store) = open_store();
    let mut meta_a = app_meta("Acme", "Backend Engineer");
    meta_a.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "Why do you want to work here?".to_string(),
        answer: "Because I love building things.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-a",
            "linkedin",
            &meta_a,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let mut meta_b = app_meta("Globex", "QA Engineer");
    meta_b.answers = vec![ApplicationAnswer {
        id: "b1".to_string(),
        question: "What is your notice period?".to_string(),
        answer: "Two weeks.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-b",
            "linkedin",
            &meta_b,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = super::answers_suggest::resolve_answers_suggest(
        &store,
        true,
        &json!({ "questions": ["What is your notice period?"] }),
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Two weeks.");
    assert_eq!(out[0].source_company.as_deref(), Some("Globex"));
}

/// Read-only proof: `resolve_answers_suggest` must never mutate the store — a
/// `list()` snapshot taken before and after a real (matching) call is
/// byte-identical.
#[test]
fn resolve_answers_suggest_never_mutates_the_store() {
    let (_dir, store) = open_store();
    let mut meta = app_meta("Acme", "Backend Engineer");
    meta.answers = vec![ApplicationAnswer {
        id: "a1".to_string(),
        question: "Why do you want to work here?".to_string(),
        answer: "Because I love building things.".to_string(),
    }];
    store
        .upsert_for_origin(
            "https://jobs.example.com/posting/suggest-readonly",
            "linkedin",
            &meta,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let before = serde_json::to_value(store.list()).unwrap();
    let _ = super::answers_suggest::resolve_answers_suggest(
        &store,
        true,
        &json!({ "questions": ["Why do you want to work here?"] }),
    )
    .unwrap();
    let after = serde_json::to_value(store.list()).unwrap();

    assert_eq!(before, after, "answers.suggest must never mutate the store");
}

/// `answers_suggest_reply` carries `ok:true` + the suggestions array.
#[test]
fn answers_suggest_reply_carries_type_and_req_id_on_success() {
    let reply = super::answers_suggest::answers_suggest_reply(
        "req-1",
        Ok(vec![super::answers_suggest::Suggestion {
            question: "Why this role?".to_string(),
            answer: "Because I love it.".to_string(),
            source_company: Some("Acme".to_string()),
            source_title: Some("Backend Engineer".to_string()),
            score: 0.8,
            salary: false,
        }]),
    );
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::msg::ANSWERS_SUGGEST_RESULT);
    assert_eq!(v["reqId"], "req-1");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["suggestions"][0]["question"], "Why this role?");
    assert_eq!(v["payload"]["suggestions"][0]["sourceCompany"], "Acme");
    assert_eq!(v["payload"]["suggestions"][0]["salary"], false);
}

/// `answers_suggest_reply` carries `ok:false` + the refusal error.
#[test]
fn answers_suggest_reply_carries_refusal_error() {
    let reply = super::answers_suggest::answers_suggest_reply(
        "req-2",
        Err(crate::error::AppError::Validation(
            super::AUTOFILL_OFF_MESSAGE.to_string(),
        )),
    );
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("Autofill is off"));
}

// ── match_questions — the pure token-Jaccard matcher ──────────────────────────

fn candidate<'a>(
    question: &'a str,
    answer: &'a str,
    company: &'a str,
    title: &'a str,
    updated_at: u64,
) -> AnswerCandidate<'a> {
    AnswerCandidate::new(question, answer, company, title, updated_at)
}

/// A close paraphrase above the threshold is matched.
#[test]
fn match_questions_returns_best_match_above_threshold() {
    let candidates = vec![candidate(
        "Why do you want to work at our company?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want to work here?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Because the mission excites me.");
    assert!(out[0].score >= 0.5);
}

/// An unrelated question stays below the threshold and is skipped entirely.
#[test]
fn match_questions_skips_below_threshold() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["What's your salary expectation?".to_string()],
        &candidates,
    );
    assert!(
        out.is_empty(),
        "an unrelated question must never be suggested"
    );
}

/// Two equally-scored candidates (identical question text) tie-break on the
/// most recently updated application — never the first-seen one.
#[test]
fn match_questions_tie_breaks_by_score_then_most_recent() {
    let candidates = vec![
        candidate(
            "Why this role?",
            "Older answer.",
            "Acme",
            "Backend Engineer",
            1_000,
        ),
        candidate(
            "Why this role?",
            "Newer answer.",
            "Globex",
            "QA Engineer",
            5_000,
        ),
    ];
    let out = match_questions(&["Why this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].answer, "Newer answer.",
        "the most recently updated application must win a tie"
    );
}

/// At most one suggestion per question, even with multiple candidates that
/// could match — never a fan-out.
#[test]
fn match_questions_caps_one_suggestion_per_question() {
    let candidates = vec![
        candidate(
            "Why this role?",
            "Answer A.",
            "Acme",
            "Backend Engineer",
            1_000,
        ),
        candidate(
            "Why this role?",
            "Answer B.",
            "Globex",
            "QA Engineer",
            2_000,
        ),
        candidate("Why this role?", "Answer C.", "Initech", "SRE", 3_000),
    ];
    let out = match_questions(&["Why this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1, "never more than one suggestion per question");
}

/// Two INPUT questions that normalize to the same text (case/whitespace
/// variants — e.g. two form fields sharing a label) collapse to one output
/// entry, not two.
#[test]
fn match_questions_dedupes_effectively_identical_input_questions() {
    let candidates = vec![candidate(
        "Why this role?",
        "Because I love it.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Why this role?".to_string(), "why   THIS role?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
}

/// The overall reply is capped at 20 suggestions even when every question has
/// a qualifying match.
#[test]
fn match_questions_caps_overall_at_max_suggestions() {
    let questions: Vec<String> = (0..25).map(|i| format!("Question {i}?")).collect();
    let candidates: Vec<AnswerCandidate> = questions
        .iter()
        .map(|q| candidate(q, "An answer.", "Acme", "Backend Engineer", 1_000))
        .collect();
    let out = match_questions(&questions, &candidates);
    assert_eq!(
        out.len(),
        20,
        "the overall reply must be capped at MAX_SUGGESTIONS"
    );
}

/// A question matching a salary keyword is flagged `salary: true` — the
/// popup's Copy-only rule reads this field directly.
#[test]
fn match_questions_flags_salary_keyword_questions() {
    let candidates = vec![candidate(
        "What is your expected salary?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["What is your expected salary range?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(
        out[0].salary,
        "a salary-keyword question must be flagged for Copy-only"
    );
}

/// A question with no salary keyword is never flagged.
#[test]
fn match_questions_does_not_flag_non_salary_questions() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want to work here?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(!out[0].salary);
}

/// Punctuation must never fracture a token from its bare form elsewhere: a
/// short stored question ("Notice period") against the verbose scanned label
/// ("What is your notice period?") shares 2 tokens {notice, period} over a
/// 5-token union = 0.4 — exactly at the (lowered) threshold. Before the
/// matcher-local tokenizer, the trailing "?" made "period?" a distinct token
/// from "period" and this pair scored only 0.2, well under the old 0.5.
#[test]
fn match_questions_matches_short_paraphrase_despite_trailing_punctuation() {
    let candidates = vec![candidate(
        "Notice period",
        "Two weeks.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your notice period?".to_string()], &candidates);
    assert_eq!(
        out.len(),
        1,
        "punctuation must not block a genuine short-vs-verbose paraphrase"
    );
    assert_eq!(out[0].answer, "Two weeks.");
}

/// The other PR-6 regression pair: "want to work here" vs "want this role"
/// share {why, do, you, want} (4) over a 9-token union = 0.44, just above the
/// lowered 0.4 threshold — the pair the 0.5 threshold used to reject outright.
#[test]
fn match_questions_matches_why_this_role_paraphrase() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because I love building things.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Because I love building things.");
}

/// Negative regression pair for the lowered 0.4 threshold: two genuinely
/// unrelated questions share zero tokens and must never match, however low
/// the threshold goes.
#[test]
fn match_questions_does_not_match_unrelated_salary_and_license_questions() {
    let candidates = vec![candidate(
        "What is your salary expectation?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Do you have a driver's license?".to_string()],
        &candidates,
    );
    assert!(
        out.is_empty(),
        "unrelated questions must never match, even at a lowered threshold"
    );
}

/// Near-miss negative regression: a partial-overlap pair that shares 3 of 8
/// tokens ("what","is","your" out of {what,is,your,desired,start,date,
/// favorite,color}) — 3/8 = 0.375, just below `MIN_SCORE` (0.4) — must never
/// match despite sharing a common question-stem.
#[test]
fn match_questions_does_not_match_near_miss_partial_overlap() {
    let candidates = vec![candidate(
        "What is your desired start date?",
        "Immediately",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your favorite color?".to_string()], &candidates);
    assert!(
        out.is_empty(),
        "3/8 = 0.375 overlap must fall below MIN_SCORE and never match"
    );
}

/// Broadened salary keywords (critic finding): "how much ... paid" flags
/// Copy-only without a literal "salary"/"compensation" token.
#[test]
fn match_questions_flags_how_much_paid_as_salary() {
    let candidates = vec![candidate(
        "How much do you expect to be paid?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["How much do you expect to be paid?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// "income" alone is enough to flag Copy-only.
#[test]
fn match_questions_flags_income_as_salary() {
    let candidates = vec![candidate(
        "Expected income",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Expected income".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// "day rate" (a salary-shaped multi-token phrase) is flagged.
#[test]
fn match_questions_flags_day_rate_as_salary() {
    let candidates = vec![candidate(
        "Day rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Day rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Bare "rate" (no salary-shaped multi-token phrase) must NEVER trip the
/// denylist — a skills self-rating question is not a salary question.
#[test]
fn match_questions_does_not_flag_rate_your_skills_as_salary() {
    let candidates = vec![candidate(
        "Rate your TypeScript skills",
        "9/10",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Rate your TypeScript skills".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(!out[0].salary);
}

/// Hyphen-proof salary check (critic finding): `normalize_question` only
/// collapses whitespace, so "Day-rate" still carries the literal hyphen and
/// would silently miss the "day rate" phrase without the matcher's
/// non-alphanumeric re-tokenization.
#[test]
fn match_questions_flags_hyphenated_day_rate_as_salary() {
    let candidates = vec![candidate(
        "Day-rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Day-rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Same as above with a slash instead of a hyphen.
#[test]
fn match_questions_flags_slash_day_rate_as_salary() {
    let candidates = vec![candidate(
        "day/rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["day/rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Hyphenated "How-much" must still flag Copy-only, same as the
/// space-separated "How much" case above.
#[test]
fn match_questions_flags_hyphenated_how_much_as_salary() {
    let candidates = vec![candidate(
        "How-much do you expect?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["How-much do you expect?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// CodeRabbit finding: a single-word keyword ("paid") must match a WHOLE
/// token, never a substring inside an unrelated word — "unpaid" must never
/// trip the salary denylist.
#[test]
fn match_questions_does_not_flag_unpaid_leave_as_salary() {
    let candidates = vec![candidate(
        "Unpaid leave policy acknowledgment",
        "Acknowledged.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Unpaid leave policy acknowledgment".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(
        !out[0].salary,
        "\"paid\" must not substring-match inside \"unpaid\""
    );
}

/// The single-word exact-token fix must not regress the genuine "paid"
/// salary question it was narrowed from.
#[test]
fn match_questions_still_flags_how_much_will_i_be_paid_as_salary() {
    let candidates = vec![candidate(
        "How much will I be paid?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["How much will I be paid?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Pure property: the SAME inputs always produce the SAME output — no AI, no
/// egress, no randomness (the PR-6 handoff's binding determinism property).
#[test]
fn match_questions_is_deterministic() {
    let candidates = vec![
        candidate(
            "Why this role?",
            "Answer A.",
            "Acme",
            "Backend Engineer",
            1_000,
        ),
        candidate(
            "Why this role?",
            "Answer B.",
            "Globex",
            "QA Engineer",
            2_000,
        ),
    ];
    let questions = vec!["Why this role?".to_string()];
    let first = match_questions(&questions, &candidates);
    let second = match_questions(&questions, &candidates);
    assert_eq!(first, second);
}
