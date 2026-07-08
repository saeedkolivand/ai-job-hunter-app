# Extension domain (browser extension + desktop bridge)

Last updated: 2026-06-24 (cross-distro + Flatpak support)

Owned by `extension-author` / `extension-reviewer`; security co-reviewed by `tauri-security-reviewer`.

## Primary paths

| Area                   | Path                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| Extension app (MV3)    | `apps/extension/src/` — background/service worker, popup, content scripts, `lib/bridge.ts`, `lib/messages.ts` |
| Desktop bridge (Rust)  | `apps/desktop/src-tauri/src/extension_bridge/` — frame dispatch, token gate, import handler                   |
| Shared wire protocol   | `packages/shared/src/ipc/extension-protocol-constants.ts` + `extension-protocol.ts`                           |
| Store policy checklist | `.claude/skills/extension-standards/SKILL.md`                                                                 |

## Auth model

A WebSocket (or native-messaging) connection is **not** authorized on handshake. `connected`/authorized flips **only** after a frame passes the per-frame 256-bit pairing token gate. A bad-token frame gets `unauthorized` + socket close (`FrameDecision::Unauthorized`). The `auth` message type (`EXTENSION_MESSAGE_TYPES.auth`) lets the extension verify its token on connect before sending import frames.

The auth boundary lives in `extension_bridge/mod.rs` (`classify_frame` / the socket loop). The extension side: sends `auth` on open, enters `bad_token` phase on `error==='unauthorized'`, and calls `resetForNewToken()` for recovery.

## Connection phases

The popup renders one of five link states (glossary: CONTEXT.md "Connection phase"):

- `app_not_running` — desktop app unreachable.
- `searching` — probing / reconnecting; **also where a transient auth-handshake timeout folds in** (transient → retry).
- `not_paired` — no pairing token stored.
- `bad_token` — a **wrong** token, surfaced **only** via the server's explicit `error==='unauthorized'` reply before it closes the socket — never inferred from a timeout.
- `connected` — the auth handshake succeeded.

**A handshake _timeout_ is a transport failure (→ `searching`/reconnect), never `bad_token`** — treating a timeout as a bad token would falsely accuse a good token, and the reconnect is self-correcting. There is deliberately **no `auth_timeout` phase**: a timeout is transient, not a distinct terminal state.

## Transport

Primary: **native messaging** (browser spawns desktop `--native-host` as a stdio relay; immune to Firefox HTTPS-Only Mode `ws://→wss://` upgrade). Fallback: loopback WebSocket. Both transports share the same wire envelope defined in the shared protocol constants.

### Native-messaging host registration

The desktop bridge (`extension_bridge/register.rs`) writes the browser's native-messaging manifest to OS-specific directories on every startup (idempotent, best-effort, non-fatal on failure). **Browser detection** across native paths, Snap, and Flatpak installs populates the manifest with the current exe path, so manifest tracks app moves/updates automatically.

Native-messaging registry locations are OS- and sandboxing-aware. See `apps/desktop/src-tauri/src/extension_bridge/register.rs` and `apps/desktop/src-tauri/src/platform/chrome/mod.rs` for per-platform registry paths, Flatpak sandbox handling, and fallback WebSocket bridge logic for sandboxed browsers.

## Protocol lockstep rule

A new message type or field MUST be added to the TS shared constants (`EXTENSION_MESSAGE_TYPES`) and the Rust `msg` constants in `extension_bridge/mod.rs` in the **same change**. The TS side is the wire spec; Rust must follow. A parity test in `extension_bridge/test.rs` pins the constants.

## Import flow

Single unified import: `background.ts::runImport` always tries DOM capture first (`scripting.executeScript` → `content.js`), falls back to URL-only if capture is blocked (restricted pages). No user-visible mode selection — one **Import this job** button. The bridge side (`extension_bridge/mod.rs::handle_import`) acquires the shared `"scrape_url"` rate-limiter slot (same key/constants as `scrape_url` IPC: 30 req/60 s, 2 concurrent — see `limits/mod.rs` `SCRAPE_RATE_MAX` / `SCRAPE_CONCURRENCY_MAX`).

## Store policy

Chrome Web Store + Firefox AMO: MV3, no remote code, least-privilege permissions, single-purpose, honest metadata, privacy/data disclosure. Full pre-submission checklist in `.claude/skills/extension-standards/SKILL.md`.

## Agent count

The full fleet has 23 agents (21 domain agents + `cleanup` + `project-steward`). See `CLAUDE.md` routing table for the complete list.
