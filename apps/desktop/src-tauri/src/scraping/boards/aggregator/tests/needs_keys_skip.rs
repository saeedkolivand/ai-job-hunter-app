use super::super::*;
use super::support::*;

// ── needs-keys skip classification (aggregator) ───────────────────────────────

/// Drive an async board `search` from a sync (`#[test]`) body so the keyring lock
/// guard is never held across an `.await` suspend point clippy can observe
/// (`await_holding_lock`) — same harness the comeet board uses.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

/// An aggregator with NO keys at all reports `needs_keys()` again.
///
/// This restores the original contract. It was inverted while freehire sat
/// under the keyed tiers as an always-on keyless floor: skipping the board then
/// would have skipped the one provider a fresh install actually had, so the skip
/// became unreachable. freehire is now its own catalog board, chosen explicitly,
/// so every provider left here is key-backed and a keyless search really can
/// only come back empty — which is exactly what `needs-keys` is for.
///
/// A keyring READ FAULT is still not a needs-keys skip — that is
/// `aggregator_store_read_failure_is_not_a_needs_keys_skip`.
#[test]
fn a_keyless_aggregator_is_skipped_for_keys_again() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    assert!(
        AggregatorScraper.needs_keys(),
        "every remaining provider is key-backed, so a keyless board must skip"
    );
}

/// A configured aggregator (Adzuna keys present) → `needs_keys() == false`, so the
/// board runs normally and is NOT skipped.
#[test]
fn aggregator_does_not_need_keys_when_configured() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    for slot in adzuna_slots() {
        keyring_core::Entry::new(crate::credentials::SERVICE, &slot)
            .unwrap()
            .set_password("configured")
            .unwrap();
    }

    assert!(
        !AggregatorScraper.needs_keys(),
        "a configured aggregator (both Adzuna keys present) must report needs_keys()==false"
    );

    clear_aggregator_slots();
}

/// A keyring READ FAILURE must NOT be classified as `needs-keys`: `needs_keys()`
/// returns false so the board still runs, and `search` surfaces the fault as a
/// board error (credential store unavailable) rather than a silent empty.
#[test]
fn aggregator_store_read_failure_is_not_a_needs_keys_skip() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &adzuna_slots()[0]).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "keyring backend unavailable".to_string(),
    ));

    assert!(
        !AggregatorScraper.needs_keys(),
        "a store read failure must not be treated as a needs-keys skip"
    );

    clear_aggregator_slots();
}

/// Companion to the classification test: a store read failure makes `search`
/// return a board error naming the store as unavailable — no silent empty, no
/// network call (the guard short-circuits before any provider is built).
#[test]
fn aggregator_search_surfaces_store_read_failure_as_error() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &adzuna_slots()[0]).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "keyring backend unavailable".to_string(),
    ));

    let result = block_on(AggregatorScraper.search(make_input(), make_ctx()));
    assert!(
        result.is_err(),
        "a credential-store read failure must surface as a board error, not Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential store unavailable"),
        "the diagnostic must name the store as unavailable; got: {msg}"
    );

    clear_aggregator_slots();
}

/// Arm a non-NoEntry failure on the APIFY token slot only (Adzuna + JSearch stay
/// absent — `NoEntry` → `Ok(None)`). `keyring_core::mock::Cred::set_error` is a
/// ONE-SHOT: the induced error is returned and cleared on the very next entry
/// method call, then the mock reads cleanly again — so callers must arm it
/// immediately before the single probe they intend to exercise, never reuse one
/// arm across two reads (a prior version of this test armed once and then called
/// both `needs_keys()` and `search()` against it, so the second call silently saw
/// a clean read and the test asserted the wrong thing without failing to compile).
fn arm_apify_slot_fault() {
    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &apify_slot()).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "keyring backend unavailable".to_string(),
    ));
}

/// Consistency guard: `aggregator_has_configured_provider` counts the Apify
/// provider, so `aggregator_store_error` must probe the Apify token slot too. A
/// keyring READ FAILURE on the APIFY token slot ALONE (Adzuna + JSearch merely
/// absent) must classify as a store error — `needs_keys()` false — NOT a
/// misleading `needs-keys` skip.
#[test]
fn aggregator_apify_slot_read_failure_is_not_a_needs_keys_skip() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    arm_apify_slot_fault();

    assert!(
        !AggregatorScraper.needs_keys(),
        "an Apify-slot store fault (others absent) must NOT be a needs-keys skip"
    );

    clear_aggregator_slots();
}

/// Companion to the classification test above: an APIFY-slot-only store fault
/// (Adzuna + JSearch merely absent) makes `search` return a board error naming
/// the store as unavailable — NOT a silent `Ok(empty)`. Arms its own fresh fault
/// (see [`arm_apify_slot_fault`] doc — the mock error is one-shot, so this must
/// NOT share an arm with the `needs_keys()` probe above).
#[test]
fn aggregator_apify_slot_read_failure_surfaces_as_search_error() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    arm_apify_slot_fault();

    let result = block_on(AggregatorScraper.search(make_input(), make_ctx()));
    assert!(
        result.is_err(),
        "an Apify-slot store fault must surface as a board error, not Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential store unavailable"),
        "the diagnostic must name the store as unavailable; got: {msg}"
    );

    clear_aggregator_slots();
}

/// Arm a non-NoEntry failure on the JOOBLE key slot only (Adzuna + JSearch +
/// Apify stay absent — `NoEntry` → `Ok(None)`). One-shot — see the doc comment
/// on [`arm_apify_slot_fault`] for why callers must arm immediately before the
/// single probe they intend to exercise, never reuse one arm across two reads.
fn arm_jooble_slot_fault() {
    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &jooble_slot()).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "keyring backend unavailable".to_string(),
    ));
}

/// Consistency guard (this PR's regression risk): `aggregator_has_configured_provider`
/// counts the Jooble provider, so a Jooble-only configured key must clear the
/// needs-keys skip — otherwise the aggregator board is wrongly skipped when only
/// a Jooble key is set.
#[test]
fn aggregator_does_not_need_keys_when_only_jooble_configured() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    keyring_core::Entry::new(crate::credentials::SERVICE, &jooble_slot())
        .unwrap()
        .set_password("configured")
        .unwrap();

    assert!(
        !AggregatorScraper.needs_keys(),
        "a Jooble-only configured aggregator must report needs_keys()==false"
    );

    clear_aggregator_slots();
}

/// `aggregator_store_error` must probe the Jooble slot too. A keyring READ
/// FAILURE on the JOOBLE slot ALONE (others merely absent) must classify as a
/// store error — `needs_keys()` false — NOT a misleading `needs-keys` skip.
#[test]
fn aggregator_jooble_slot_read_failure_is_not_a_needs_keys_skip() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    arm_jooble_slot_fault();

    assert!(
        !AggregatorScraper.needs_keys(),
        "a Jooble-slot store fault (others absent) must NOT be a needs-keys skip"
    );

    clear_aggregator_slots();
}

/// Companion to the classification test above: a JOOBLE-slot-only store fault
/// (others merely absent) makes `search` return a board error naming the store
/// as unavailable — NOT a silent `Ok(empty)`.
#[test]
fn aggregator_jooble_slot_read_failure_surfaces_as_search_error() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    arm_jooble_slot_fault();

    let result = block_on(AggregatorScraper.search(make_input(), make_ctx()));
    assert!(
        result.is_err(),
        "a Jooble-slot store fault must surface as a board error, not Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("credential store unavailable"),
        "the diagnostic must name the store as unavailable; got: {msg}"
    );

    clear_aggregator_slots();
}
