//! Rust ↔ TS protocol parity tests — redistributed from the crate-level `test.rs` (R8 relief).
//!
//! The parity test mirrors the Feature-1 stage-registry approach: it reads the shared TS protocol
//! source (`packages/shared/src/ipc/extension-protocol-constants.ts`) as text and asserts every
//! Rust message-type constant here appears as the exact string literal on the TS side. If either
//! side renames a wire `type` without the other, this fails — the two can't drift. The same test
//! also covers the two `answer.assist` refusal sentinels, which a client is allowed to MATCH
//! rather than merely display, so a one-sided reword of either string is caught the same way a
//! renamed wire type is.

use super::super::{answer_assist, applied_check_batch, PROTOCOL_VERSION};
use super::*;

/// Path from this crate's manifest dir to the shared TS protocol constants
/// source — the zod-free module that holds the `EXTENSION_MESSAGE_TYPES`
/// literal strings (`extension-protocol.ts` only re-exports them).
const TS_PROTOCOL: &str = "../../../packages/shared/src/ipc/extension-protocol-constants.ts";

fn ts_protocol_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(TS_PROTOCOL);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()))
}

#[test]
fn message_type_constants_match_ts() {
    let ts = ts_protocol_source();
    // Each Rust constant must appear as a single-quoted string literal in the TS
    // `EXTENSION_MESSAGE_TYPES` map (e.g. `import.request: 'import.request'`).
    for literal in [
        HELLO,
        CHALLENGE,
        AUTH,
        AUTH_OK,
        UPDATE_REQUIRED,
        TOKEN_REVOKED,
        IMPORT_REQUEST,
        IMPORT_RESULT,
        PROFILE_GET,
        PROFILE_RESULT,
        MATCH_LIVE,
        MATCH_RESULT,
        APPLIED_CHECK,
        APPLIED_RESULT,
        STATUS_UPDATE,
        STATUS_RESULT,
        AUTOTRACK_CHECK,
        AUTOTRACK_RESULT,
        AUTOFILL_CHECK,
        AUTOFILL_RESULT,
        ANSWERS_SAVE,
        ANSWERS_RESULT,
        ANSWERS_SUGGEST,
        ANSWERS_SUGGEST_RESULT,
        ANSWER_ASSIST,
        ANSWER_ASSIST_RESULT,
        ASSIST_CHUNK,
        ASSIST_DONE,
        ASSIST_CANCEL,
        // PR1 (extension read tier): the extension itself now sends these four —
        // Read-only, Autofill-gated, reply-capped (see `msg::AGENT_QUERY`'s doc) —
        // so they belong in the parity-tested set for the first time.
        AGENT_QUERY,
        AGENT_RESULT,
        AGENT_CALL,
        AGENT_CALL_RESULT,
        // PR1 — new settings.get/settings.set verbs (R7).
        SETTINGS_GET,
        SETTINGS_RESULT,
        SETTINGS_SET,
        // PR2 (documents into ATS) — the extension itself now sends this dedicated verb pair,
        // outside the generic agent.query/agent.call tier.
        DOCUMENT_EXPORT,
        DOCUMENT_RESULT,
        // PR3 (Check-fit on the page) — batch form of applied.check for a results-listing page.
        APPLIED_CHECK_BATCH,
        APPLIED_BATCH_RESULT,
    ] {
        let needle = format!("'{literal}'");
        assert!(
            ts.contains(&needle),
            "wire type {literal:?} (Rust) not found as {needle} in extension-protocol-constants.ts — \
             the Rust msg:: constants drifted from the shared TS EXTENSION_MESSAGE_TYPES"
        );
    }

    // Same shape, second hand-enumerated list: the two `answer.assist` REFUSAL
    // SENTINELS (ADR-044 decision 8). These are not wire `type`s — they are
    // fixed `error` TEXT, and the sentinel IS the code (no `code` field was
    // added), so a client that wants to say WHERE to turn the feature back on
    // has to match the string. Moving them to the shared TS module pins
    // nothing by itself; enumerating them HERE is what makes a one-sided
    // reword fail, exactly like a renamed wire type does above.
    for literal in [
        answer_assist::AI_ASSIST_OFF_MESSAGE,
        answer_assist::NO_PROVIDER_MESSAGE,
    ] {
        let needle = format!("'{literal}'");
        assert!(
            ts.contains(&needle),
            "refusal sentinel {literal:?} (Rust) not found as {needle} in \
             extension-protocol-constants.ts — the Rust answer_assist sentinels drifted from the \
             shared TS EXTENSION_AI_ASSIST_OFF_MESSAGE/EXTENSION_NO_PROVIDER_MESSAGE, so a client \
             matching the shared constant would stop recognizing the refusal"
        );
    }
}

/// Numeric companion to [`message_type_constants_match_ts`] for the ONE
/// number the two sides now share: the shared TS
/// `EXTENSION_ANSWER_ASSIST_MAX_CHARS` advertises the ceiling an over-large
/// `maxChars` is clamped to, and that ceiling is
/// [`super::answer_assist::DRAFT_CAP`]. Nothing enforces the constant on the
/// wire (deliberately — see the schema's doc: a `.max()` there would refuse a
/// legitimate draft over a number), so if `DRAFT_CAP` moved and the TS
/// constant did not, the shared constant would quietly become a lie a client
/// has no way to detect. The trailing `;` is in the needle so `= 4000` can
/// never prefix-match a future `= 40000`, same discipline as
/// [`protocol_version_matches_ts`].
#[test]
fn answer_assist_max_chars_matches_ts() {
    let ts = ts_protocol_source();
    let cap = answer_assist::DRAFT_CAP;
    let needle = format!("EXTENSION_ANSWER_ASSIST_MAX_CHARS = {cap};");
    assert!(
        ts.contains(&needle),
        "Rust DRAFT_CAP ({cap}) not found as `{needle}` in extension-protocol-constants.ts — the \
         desktop clamp for answer.assist's maxChars drifted from the shared TS constant"
    );
}

/// Numeric companion to [`message_type_constants_match_ts`] for
/// `applied.check.batch`'s (PR3) URL cap: the shared TS
/// `MAX_APPLIED_CHECK_BATCH_URLS` doc claims to mirror
/// [`super::applied_check_batch::MAX_BATCH_URLS`] "pinned by a parity test,
/// same discipline as every other wire constant" — without this test that
/// claim was false, so a one-sided change to either cap would silently drift
/// from the extension's client-side pre-cap. The trailing `;` is in the
/// needle so `= 50` can never prefix-match a future `= 500`, same discipline
/// as [`answer_assist_max_chars_matches_ts`].
#[test]
fn max_batch_urls_matches_ts() {
    let ts = ts_protocol_source();
    let cap = applied_check_batch::MAX_BATCH_URLS;
    let needle = format!("MAX_APPLIED_CHECK_BATCH_URLS = {cap};");
    assert!(
        ts.contains(&needle),
        "Rust MAX_BATCH_URLS ({cap}) not found as `{needle}` in extension-protocol-constants.ts — \
         the desktop's applied.check.batch URL cap drifted from the shared TS constant"
    );
}

/// Numeric parity companion to [`message_type_constants_match_ts`]: the
/// message-type test only pins the `msg::*` STRING literals — it says nothing
/// about the handshake's numeric `PROTOCOL_VERSION`. A one-sided bump (Rust
/// bumps to 3 but TS stays at 2, or vice versa) would silently miscalibrate the
/// force-cutover gate (`advance_hello`'s `protocol < PROTOCOL_VERSION` check) —
/// this pins the exact literal on both sides. The needle includes the trailing
/// `;` so `= 2` can never prefix-match a future `= 20` (etc).
#[test]
fn protocol_version_matches_ts() {
    let ts = ts_protocol_source();
    let needle = format!("EXTENSION_PROTOCOL_VERSION = {};", PROTOCOL_VERSION);
    assert!(
        ts.contains(&needle),
        "Rust PROTOCOL_VERSION ({PROTOCOL_VERSION}) not found as `{needle}` in \
         extension-protocol-constants.ts — the numeric handshake protocol version \
         drifted from the TS EXTENSION_PROTOCOL_VERSION"
    );
}

#[test]
fn reserved_types_are_distinct() {
    // Every wire type must be a distinct string.
    let all = [
        HELLO,
        CHALLENGE,
        AUTH,
        AUTH_OK,
        UPDATE_REQUIRED,
        TOKEN_REVOKED,
        IMPORT_REQUEST,
        IMPORT_RESULT,
        PROFILE_GET,
        PROFILE_RESULT,
        MATCH_LIVE,
        MATCH_RESULT,
        APPLIED_CHECK,
        APPLIED_RESULT,
        STATUS_UPDATE,
        STATUS_RESULT,
        AUTOTRACK_CHECK,
        AUTOTRACK_RESULT,
        AUTOFILL_CHECK,
        AUTOFILL_RESULT,
        ANSWERS_SAVE,
        ANSWERS_RESULT,
        ANSWERS_SUGGEST,
        ANSWERS_SUGGEST_RESULT,
        ANSWER_ASSIST,
        ANSWER_ASSIST_RESULT,
        ASSIST_CHUNK,
        ASSIST_DONE,
        ASSIST_CANCEL,
        AGENT_QUERY,
        AGENT_RESULT,
        AGENT_CALL,
        AGENT_CALL_RESULT,
        SETTINGS_GET,
        SETTINGS_RESULT,
        SETTINGS_SET,
        DOCUMENT_EXPORT,
        DOCUMENT_RESULT,
    ];
    let set: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(set.len(), all.len(), "wire type constants must be unique");
}

#[test]
fn applications_changed_event_name_is_stable() {
    // Pinned so the frontend slice's subscription string can rely on it.
    assert_eq!(crate::events::APPLICATIONS_CHANGED, "applications:changed");
}

#[test]
fn extension_bridge_changed_event_name_is_stable() {
    // Pinned so the renderer's `onChanged` subscription string can rely on it.
    assert_eq!(
        crate::events::EXTENSION_BRIDGE_CHANGED,
        "extensionBridge:changed"
    );
}
