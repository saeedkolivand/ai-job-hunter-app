//! `CancelRegistry` behaviour pins. These are the invariants the scraper engine
//! used to own inline (`scraping::engine::test`) — restated here against the
//! extracted registry so a future refactor of either side has to keep them.

use tokio_util::sync::CancellationToken;

use super::CancelRegistry;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// The core path: a token registered before the run starts is reachable by id,
/// so a cancel arriving mid-run reaches the RUNNING token (not a copy).
#[test]
fn cancel_reaches_a_registered_token() {
    block_on(async {
        let registry = CancelRegistry::new();
        let token = CancellationToken::new();
        registry.register("job-1", token.clone()).await;

        assert!(!token.is_cancelled());
        registry.cancel("job-1").await;
        assert!(
            token.is_cancelled(),
            "cancel(id) must fire the token the run is actually awaiting"
        );
    });
}

/// An unknown id is a benign no-op — never a panic, never an insert.
#[test]
fn cancel_of_an_unknown_id_is_a_no_op() {
    block_on(async {
        let registry = CancelRegistry::new();
        registry.cancel("never-registered").await;
        assert_eq!(registry.live_count().await, 0, "no phantom slot inserted");
    });
}

/// Double-cancel is idempotent, and cancelling does NOT remove the slot: the
/// run has to be able to observe `is_cancelled()` when it finally gets there.
#[test]
fn double_cancel_is_idempotent_and_leaves_the_slot_in_place() {
    block_on(async {
        let registry = CancelRegistry::new();
        let token = CancellationToken::new();
        registry.register("job-2", token.clone()).await;

        registry.cancel("job-2").await;
        registry.cancel("job-2").await;
        assert!(token.is_cancelled());
        assert_eq!(
            registry.live_count().await,
            1,
            "cancel must not remove the slot — the late-arriving run reuses it \
             and sees is_cancelled()"
        );

        // The owner is still the single remover.
        registry.unregister("job-2").await;
        assert_eq!(registry.live_count().await, 0);
    });
}

/// The cancel-before-the-run-starts window: a cancel that lands first must make
/// the mint-fresh path unreachable, so the run reuses the CANCELLED slot.
#[test]
fn a_cancel_before_the_run_starts_is_observed_by_get_or_register() {
    block_on(async {
        let registry = CancelRegistry::new();
        registry.register("job-3", CancellationToken::new()).await;
        registry.cancel("job-3").await;

        let (token, we_minted) = registry.get_or_register("job-3").await;
        assert!(!we_minted, "the pre-registered slot must be reused");
        assert!(
            token.is_cancelled(),
            "the reused token carries the cancel that landed before the run started"
        );
    });
}

/// With no pre-registration, `get_or_register` mints, registers, and reports
/// ownership — so the caller knows it must remove the slot itself.
#[test]
fn get_or_register_mints_and_claims_ownership_when_absent() {
    block_on(async {
        let registry = CancelRegistry::new();
        let (token, we_minted) = registry.get_or_register("job-4").await;
        assert!(we_minted);
        assert!(!token.is_cancelled());

        // The minted token is the SAME one `cancel` reaches.
        registry.cancel("job-4").await;
        assert!(token.is_cancelled());
    });
}

/// No leak: a finished run's `unregister` empties the map, and unregister is
/// idempotent so a double-exit path can't panic or resurrect a slot.
#[test]
fn a_finished_run_leaves_no_registration_behind() {
    block_on(async {
        let registry = CancelRegistry::new();
        registry.register("job-5", CancellationToken::new()).await;
        registry.register("job-6", CancellationToken::new()).await;
        assert_eq!(registry.live_count().await, 2);

        registry.unregister("job-5").await;
        registry.unregister("job-5").await; // idempotent
        assert_eq!(registry.live_count().await, 1);

        registry.unregister("job-6").await;
        assert_eq!(registry.live_count().await, 0);
    });
}

/// Re-registering the same id replaces the token (last writer wins), and the
/// STALE token is no longer reachable — a cancel must never fire a token that
/// belongs to a previous run of the same id.
#[test]
fn re_registering_an_id_replaces_the_token() {
    block_on(async {
        let registry = CancelRegistry::new();
        let stale = CancellationToken::new();
        let live = CancellationToken::new();
        registry.register("job-7", stale.clone()).await;
        registry.register("job-7", live.clone()).await;

        registry.cancel("job-7").await;
        assert!(live.is_cancelled());
        assert!(
            !stale.is_cancelled(),
            "the replaced token must be unreachable"
        );
    });
}

/// Cancellation is per-id: firing one job must not touch a sibling's token.
#[test]
fn cancel_is_scoped_to_one_id() {
    block_on(async {
        let registry = CancelRegistry::new();
        let a = CancellationToken::new();
        let b = CancellationToken::new();
        registry.register("job-a", a.clone()).await;
        registry.register("job-b", b.clone()).await;

        registry.cancel("job-a").await;
        assert!(a.is_cancelled());
        assert!(!b.is_cancelled(), "a sibling job must be untouched");
    });
}
