//! Browser-extension ⇄ desktop bridge — a loopback-only WebSocket server.
//!
//! Feature 2: the browser extension's "Save this job" button opens a WS to the
//! desktop app and sends an [`import.request`](shared `extension-protocol.ts`)
//! frame. The desktop scrapes/parses the posting, creates a [`crate::applications`]
//! aggregate from it (Application only — an import is a pursuit, not a discovery,
//! so it does NOT enter the postings cache / Jobs feed), and replies with
//! `import.result`.
//!
//! ## Security model (layered — see [`auth`])
//! 1. **Loopback only** — the listener binds `127.0.0.1`; no LAN/remote reach.
//! 2. **Origin allowlist** (defense-in-depth, not the primary boundary) — the
//!    WS handshake's `Origin` must be an allowed extension origin: Chrome is
//!    pinned by store id (`chrome-extension://<id>` in
//!    [`auth::ALLOWED_EXTENSION_IDS`]); Firefox is accepted by UUID **shape**
//!    (`moz-extension://<uuid>`), since its per-install internal UUID is
//!    unknowable in advance — see [`auth::is_allowed_origin`]. A dev override
//!    (`platform::config::extension_dev_origins`) admits a locally-loaded
//!    extension. The mutual HMAC handshake below (3) is what actually
//!    authenticates.
//! 3. **Mutual HMAC challenge-response (protocol v2)** — the pairing token is
//!    NEVER transmitted. On connect the extension sends [`msg::HELLO`]
//!    `{protocol, clientNonce}`; the desktop replies [`msg::CHALLENGE`]
//!    `{serverNonce}`; the extension sends [`msg::AUTH`] `{proof}` where
//!    `proof = HMAC-SHA256(token, CLIENT_MSG)`; the desktop verifies it
//!    **constant-time** ([`handshake::verify_client_proof`]) and, on success,
//!    replies [`msg::AUTH_OK`] `{serverProof}` (`HMAC-SHA256(token, SERVER_MSG)`)
//!    so the extension can prove the desktop is genuine (not a port-squatter).
//!    `connected` flips true ONLY once the client proof verifies — the WS
//!    handshake and the `hello`/`challenge` exchange alone do NOT authorize.
//!    After auth the socket is session-authenticated: `import.request` /
//!    `profile.get` frames carry NO token (see [`advance_frame`]). A first frame
//!    that is not a valid protocol-2 `hello` (an old extension's legacy
//!    `{type:'auth', token}` frame, a lower protocol) gets [`msg::UPDATE_REQUIRED`]
//!    and the socket closes — a hard cutover with no dual-support path.
//! 4. **Size cap** — frames over [`MAX_FRAME_BYTES`] are rejected.
//! 5. **URL/SSRF guard** — the imported `url` is normalized (http(s) only) and
//!    run through [`auth::is_safe_public_host`] (rejects loopback/private/
//!    link-local/`*.local`) before any fetch.
//!
//! ## Layering
//! This is an **L3 shell** module (like `commands`/`tray`/`updater`): it holds an
//! `AppHandle`, emits Tauri events, and reaches down into L1 (`applications`,
//! `scraping`) — never the reverse. Server startup is fire-and-forget with
//! graceful failure: a bind error logs + disables the bridge but never blocks app
//! boot.

use std::path::Path;

/// ADR-038 §2's generic `agent.call` dispatch tier — `Effect::Read` (Phase 2)
/// and `Effect::Reversible` (Phase 4) dispatch unconditionally,
/// `Effect::Irreversible` (Phase 3) only after a `--confirm` ceremony; see
/// its module doc for the full gate.
mod agent_call;
/// `ajh-tauri agent <verb>` — the CLIENT half of the agent/CLI surface (issue
/// #1084 PR 1): argv parsing, the v2-handshake-carrying bridge client, and
/// process exit codes. `pub` (not plain `mod`) because `lib::
/// run_agent_cli_if_invoked` calls into it from OUTSIDE this module tree,
/// exactly like [`native_host`] below. See its own module doc.
pub mod agent_cli;
/// The `agent.query` read-only agent/CLI surface (issue #1084 PR 1) — see its
/// module doc.
mod agent_read;
/// `answer.assist` — see its module doc. `pub(crate)` for its two compose
/// budgets alone: they are sized against Anthropic's classic-thinking gate,
/// and that relationship is asserted next to the gate itself, in
/// `commands::ai_provider::anthropic`'s tests. Everything else here stays
/// `pub(super)`/private.
pub(crate) mod answer_assist;
/// `maxChars` parsing for `answer.assist` (R8 relief, PR4) — see its own module doc.
mod answer_assist_max_chars;
mod answer_assist_parse;
/// The two on-demand Prep-tab `topic`s on `answer.assist` (PR4) — see its own module doc.
mod answer_assist_topic;
mod answer_rewrite;
mod answers_save;
mod answers_suggest;
mod applied_check;
/// `applied.check.batch` → `applied.batch.result` (PR3).
mod applied_check_batch;
mod assist_registry;
pub mod auth;
/// The AUTO-flag consent gate, table-driven across `status.update`/`answers.save` — see its own
/// module doc.
#[cfg(test)]
mod auto_flag_gate_tests;
mod autofill_check;
/// `profile.get` → `profile.result` — see its own module doc.
mod autofill_profile;
mod autotrack;
/// `CallerClass` + the per-verb dispatch matrix that gates on it — see its own module doc
/// (R8 relief, PR1 — extension read tier).
mod caller_gate;
/// `document.export` → `document.result` (PR2 — documents into ATS) — see its own module doc.
mod document_export;
/// `document.export` request parsing + pure source resolution — split from `document_export` (R8 relief).
mod document_export_parse;
/// `document.export`'s dedicated per-pairing throttle — split from `document_export` (R8 relief).
mod document_export_throttle;
/// [`FrameDecision`] — see its own module doc (R8 relief, PR2).
mod frame;
/// The handshake state machine that produces a `FrameDecision` — see its own module doc.
mod frame_advance;
pub mod handshake;
mod import_flow;
/// Post-persist event + Notification Center push for `import.request` — split from `import_flow`.
mod import_flow_notify;
/// Pure posting-resolution helpers for `import.request` — split from `import_flow` (R8 relief).
mod import_flow_resolve;
mod match_live;
/// Import-time best-effort match score — split from `match_live` (R8 relief).
mod match_live_import_score;
/// Pure scoring primitives shared by `match_live` and `match_live_import_score` (R8 relief).
mod match_live_score;
/// `match.live`'s dedicated per-pairing throttle — split from `match_live` (R8 relief).
mod match_live_throttle;
/// Scoring-timeout primitives shared by `match_live`/`match_live_import_score` (R8 relief).
mod match_live_timeout;
/// Wire `type` constants (the TS-mirrored protocol table) — see its module doc.
pub mod msg;
pub mod native_host;
/// Rejected-origin + handshake-failure log noise suppression — split out of `auth` (R8 relief).
mod origin_log;
/// Offset/limit/byte-budget paging primitives shared by `agent_read`'s
/// `found-jobs` resource and `agent_call`'s generic dispatch tier — see its
/// module doc for why they live here rather than in either caller.
mod paging;
mod persist;
pub mod register;
mod req_id_cap;
/// The `token.revoked` wire surface + its no-oracle gate — see its module doc.
mod revoke;
/// `saveAnswersOnSubmit` opt-in (PR4) — see its own module doc.
mod save_answers_optin;
/// `settings.get`/`settings.set` (R7) — the extension's own opt-in switches, toggleable from the
/// paired extension — see its own module doc.
mod settings;
/// `settings.set`'s validate-then-apply-then-notify path — split from `settings` (R8 relief).
mod settings_set;
mod status_update;
mod stream;
/// The streaming compose core for `answer.assist`/rewrite — split from `stream` (R8 relief).
mod stream_compose;
/// Live cap-clamped delta forwarding for a streaming `answer.assist` — split from `stream`.
mod stream_forward;
/// Off-the-read-loop task spawning (`answer.assist`/`agent.query`/`agent.call`) — split from `stream`.
mod stream_spawn;
/// The connection read/write race primitives — split from `stream` (R8 relief).
mod stream_writer;

/// Re-exported so `answer_assist` (a sibling of [`stream`]) can keep
/// referring to it as `super::FrameSink` — see [`stream`]'s module doc for
/// the streaming relay this abstracts over.
pub(crate) use stream::FrameSink;
// Re-exported (not used directly in this file any more — `connection`/`connection_dispatch` own
// every call site now) so `agent_cli`'s tests/etc. via `crate::extension_bridge::X` keep
// resolving these by their original bare names — see `caller_gate`'s own module doc.
pub(in crate::extension_bridge) use self::caller_gate::CallerClass;
pub(in crate::extension_bridge) use self::frame::FrameDecision;
#[cfg(test)]
pub(in crate::extension_bridge) use self::frame_advance::advance_frame;
// `ConnState` has no production caller through this path any more (`connection`/`frame` import
// it directly from `frame_advance`) — only test code (e.g. `agent_cli::handshake_client`'s
// tests) still reaches it as `crate::extension_bridge::ConnState`.
#[cfg(test)]
pub(in crate::extension_bridge) use self::frame_advance::ConnState;

/// Shared test-only fixtures (`app_meta`/`sample_posting`/`open_store`/`bridge_state`) reused
/// across many submodules' own test files — see its own module doc.
#[cfg(test)]
mod test_support;

/// Refusal text for the assisted-autofill opt-in gate — shared verbatim by
/// [`resolve_profile`] (`profile.get`) and
/// [`answers_save::resolve_answers_save`] (`answers.save`), the fill/capture
/// mirror pair riding the SAME consent gate. A single constant (not two
/// copies) so the two can never drift.
pub(crate) const AUTOFILL_OFF_MESSAGE: &str =
    "Autofill is off. Turn it on in AI Job Hunter → Settings → Browser extension.";

/// Native-messaging host name — the registered identifier the browser uses to
/// spawn our relay (our exe in `--native-host` mode). MUST match the extension
/// side exactly (`apps/extension`). The host-manifest filename is this with a
/// `.json` suffix.
pub const NATIVE_HOST_NAME: &str = "app.aijobhunter.bridge";

/// On-disk host-manifest filename the browser reads to find + spawn the host.
pub const NATIVE_HOST_MANIFEST: &str = "app.aijobhunter.bridge.json";

/// Handshake protocol version carried in the `hello` frame. MUST match the TS
/// `EXTENSION_PROTOCOL_VERSION` in `packages/shared/.../extension-protocol-constants.ts`.
/// A `hello` with a lower (or absent) protocol is treated as an outdated client.
pub const PROTOCOL_VERSION: u64 = 2;

/// Hard cap on a single WS message. A job page's full `outerHTML` can run to a
/// few MB, so 8 MB matches the scraper's per-response cap
/// ([`crate::scraping::http`]) — a full-page DOM capture isn't silently dropped
/// — while still blocking a memory-exhaustion frame.
///
/// TWO consumers, not one. Besides bounding what this server will READ, it is
/// also the ceiling `agent_call::enforce_frame_cap` measures an OUTGOING
/// `agent.call` reply against (issue #1135). That second use exists because
/// tungstenite 0.30 checks `max_message_size` on the READ path only —
/// `WebSocketContext::check_max_size` runs while reassembling an INCOMING
/// message and nothing checks an outgoing one — while the CLI's own reader
/// configures this SAME constant (`agent_cli.rs`). So an over-cap reply is
/// written happily here and then dropped by the peer as a transport error the
/// CLI can only report as a content-free `connection_lost`. Raising or
/// lowering this therefore moves BOTH the accepted-request size and the
/// point at which a legitimate reply starts being refused as
/// `result_too_large`.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// The extension caller's own reply cap for `agent.query`/`agent.call` (PR1, extension read tier
/// — MCP precedent: `agent_cli::mcp::MCP_RESULT_MAX_BYTES` layers an identical tighter budget one
/// hop inside this SAME [`MAX_FRAME_BYTES`] ceiling). The CLI's own replies are unaffected — this
/// is enforced ONLY for [`CallerClass::Extension`], by `stream::spawn_agent_query`/
/// `spawn_agent_call` via `agent_read::extension_capped_reply`/`agent_call::extension_capped_reply`
/// — refuse-not-truncate, same discipline as every other cap on this bridge.
pub(super) const EXTENSION_RESULT_MAX_BYTES: usize = 256 * 1024;

/// First port tried, then the rest of the inclusive range until one binds.
const PORT_RANGE: std::ops::RangeInclusive<u16> = 47615..=47620;

/// File under the app data dir holding the persisted pairing token.
const TOKEN_FILE: &str = "extension_token";

/// File under the app data dir holding the assisted-autofill opt-in flag
/// (`"1"` = on, anything else / absent = off). Default OFF: the desktop returns
/// the contact profile for a `profile.get` only when this is on.
const AUTOFILL_OPTIN_FILE: &str = "extension_autofill_optin";

/// File under the app data dir holding the AI-answer-assist opt-in, as one
/// small JSON blob (`{"enabled":bool}`). It USED to also carry a
/// provider/model/base_url snapshot; that snapshot is gone (task #16 — a draft
/// resolves the active provider from the backend-owned
/// [`crate::ai_config::AiConfigStore`] at answer-time), so only `enabled` is
/// read/written now. An OLD file that still has the extra fields is read back
/// fine — the extras are ignored. Default OFF, absent/corrupt file → OFF (the
/// safe state).
const AI_ASSIST_OPTIN_FILE: &str = "extension_ai_assist_optin";

mod state;
pub use state::BridgeState;

mod connection;
mod connection_accept;
mod connection_dispatch;
mod server;
pub use server::start;

// ── Assisted autofill (profile.get → profile.result) — extracted to
// `autofill_profile` (R8 relief); re-exported below so this module's own
// existing reference (`handle_connection`'s `Profile` arm) keeps resolving
// unchanged. `autofill_profile`'s own tests reach the rest of that module
// directly (`use super::*;`), so nothing else needs re-exporting here.
use autofill_profile::{handle_profile, profile_outcome};

/// Manage the bridge state and register its factory-reset hook. Returns the
/// state handle so `start` can be wired right after. Mirrors the
/// `manage_resettable` pattern but is bridge-specific (it returns nothing app
/// state can't already resolve via `app.state::<BridgeState>()`).
pub fn manage(
    app: &tauri::App,
    registry: &mut crate::commands::privacy::ResetRegistry,
    data_dir: &Path,
) {
    crate::commands::privacy::manage_resettable(
        app,
        registry,
        "extension_bridge",
        BridgeState::load(data_dir),
    );
}
