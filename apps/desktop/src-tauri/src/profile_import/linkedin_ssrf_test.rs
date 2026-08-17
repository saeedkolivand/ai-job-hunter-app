//! Hermetic SSRF-guard regression test for [`fetch_page`] — split out of
//! `linkedin.rs`'s inline `#[cfg(test)] mod tests` into this sibling
//! `_test.rs` file.
//!
//! Why the split: `fetch_page` calls `has_linkedin_session()` before the
//! guarded request, which resolves `platform::config::data_dir()` and reads
//! a real cookie file from disk (diagnostic only — the result never affects
//! the outcome, but the read itself touches host state unless the data dir
//! is pointed at a scratch directory for the test's duration). R4
//! (`tests/architecture.rs::r4_env_access_only_in_platform`) bans that env
//! var's literal name from any non-`platform/**` source that ISN'T itself a
//! recognized test file (`*test.rs`/`*tests.rs`) — `linkedin.rs` doesn't
//! qualify by name (R4 scans full raw file content, not just non-test
//! `#[cfg]` regions), so mutating it inline there would fail R4 even though
//! the mutation only ever runs in `#[cfg(test)]` code. This file's name does
//! qualify, exactly like `commands/ai_provider/anthropic_tests.rs`.

use super::*;

// ── fetch_page: the fetch path must be SSRF-guarded ───────────────────────
//
// `url` is user-pasted, attacker-influenced input, so `fetch_page` must
// route it through `net::http::get_guarded_following_redirects` (IP
// validation before connecting) rather than the plain pooled `shared()`
// client. Asserting only `Err(_)` here would pass for the wrong reason —
// an unguarded client hitting an unlistened loopback port also errors
// (connection refused). Instead this test proves the stronger property: a
// REAL, listening loopback socket never receives a connection attempt at
// all, which only a pre-connect SSRF rejection (not a failed connect)
// explains.

#[tokio::test]
#[serial_test::serial]
async fn fetch_page_never_dials_the_loopback_literal_it_rejects() {
    // Scope the data dir to an empty temp dir so has_linkedin_session()'s
    // real-disk read never touches the developer's or CI runner's actual data
    // dir. The guard lives in platform/config.rs because that module is the
    // sole owner of that env var — this file never names it, which
    // is what R4 is actually asking for. It restores on drop, so a panicking
    // assertion below cannot leak the override into the next test.
    // `#[serial]` is still required: the variable is process-global and the
    // resolver's own test mutates it directly.
    let tmp = tempfile::TempDir::new().expect("create temp data dir");
    let _data_dir = crate::platform::config::DataDirGuard::set(tmp.path());

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    listener
        .set_nonblocking(true)
        .expect("set listener non-blocking");
    let port = listener.local_addr().unwrap().port();

    let result = fetch_page(&format!("http://127.0.0.1:{port}/in/x")).await;

    let err = result.unwrap_err();
    assert!(matches!(err, AppError::Network(_)), "got {err:?}");

    // Poll briefly for a connection a guarded fetch must never make. The
    // guarded rejection is a synchronous, pre-network check, so under
    // correct code this returns false almost immediately; 300ms is ample
    // margin without making the test slow.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
    let mut connected = false;
    while std::time::Instant::now() < deadline {
        if listener.accept().is_ok() {
            connected = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !connected,
        "fetch_page dialed the rejected loopback socket — the SSRF guard was bypassed"
    );
}
