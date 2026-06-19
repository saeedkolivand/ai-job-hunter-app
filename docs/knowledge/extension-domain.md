# Extension domain (browser extension + desktop bridge)

Last updated: 2026-06-19

Owned by `extension-author` / `extension-reviewer`; security co-reviewed by `tauri-security-reviewer`.

## Primary paths

| Area                   | Path                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| Extension app (MV3)    | `apps/extension/src/` — background/service worker, popup, content scripts, `lib/bridge.ts`, `lib/messages.ts` |
| Desktop bridge (Rust)  | `apps/tauri/src-tauri/src/extension_bridge/` — frame dispatch, token gate, import handler                     |
| Shared wire protocol   | `packages/shared/src/ipc/extension-protocol-constants.ts` + `extension-protocol.ts`                           |
| Store policy checklist | `.claude/skills/extension-standards/SKILL.md`                                                                 |

## Auth model

A WebSocket (or native-messaging) connection is **not** authorized on handshake. `connected`/authorized flips **only** after a frame passes the per-frame 256-bit pairing token gate. A bad-token frame gets `unauthorized` + socket close (`FrameDecision::Unauthorized`). The `auth` message type (`EXTENSION_MESSAGE_TYPES.auth`) lets the extension verify its token on connect before sending import frames.

The auth boundary lives in `extension_bridge/mod.rs` (`classify_frame` / the socket loop). The extension side: sends `auth` on open, enters `bad_token` phase on `error==='unauthorized'`, and calls `resetForNewToken()` for recovery.

## Transport

Primary: **native messaging** (browser spawns desktop `--native-host` as a stdio relay; immune to Firefox HTTPS-Only Mode `ws://→wss://` upgrade). Fallback: loopback WebSocket. Both transports share the same wire envelope defined in the shared protocol constants.

## Protocol lockstep rule

A new message type or field MUST be added to the TS shared constants (`EXTENSION_MESSAGE_TYPES`) and the Rust `msg` constants in `extension_bridge/mod.rs` in the **same change**. The TS side is the wire spec; Rust must follow. A parity test in `extension_bridge/test.rs` pins the constants.

## Store policy

Chrome Web Store + Firefox AMO: MV3, no remote code, least-privilege permissions, single-purpose, honest metadata, privacy/data disclosure. Full pre-submission checklist in `.claude/skills/extension-standards/SKILL.md`.

## Agent count

The full fleet has 23 agents (21 domain agents + `cleanup` + `project-steward`). See `CLAUDE.md` routing table for the complete list.
