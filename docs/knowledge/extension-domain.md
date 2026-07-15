# Extension domain (browser extension + desktop bridge)

Last updated: 2026-07-15 (bridge verbs roadmap, streaming AI assist, answers capture, reserved-verb pattern)

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

## Bridge verbs (reserved-verb pattern)

The bridge uses a **reserved-verb pattern**: each verb is defined in shared constants (TS + Rust), has a Zod schema for wire shape, a Rust handler with parity tests, and an extension guard in `bridge.ts`. This keeps the protocol a single source of truth and makes adding new verbs (read: modify constant + handler + test + guard) predictable and auditable.

| Verb                                                                                        | Direction                       | Consent gate                                                                              | Wire shape                                                                                                                                                        | Notes                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hello` / `challenge` / `auth` / `auth.ok` / `update.required`                              | Extension → Desktop → Extension | No gate (handshake)                                                                       | HMAC challenge-response ([ADR 0010](../adr/0010-bridge-hmac-handshake.md))                                                                                        | Mutual-auth protocol v2; no plaintext token; force cutover; `outdated` phase on version mismatch                                                                                                                                                                |
| `import.request` / `import.result`                                                          | Extension → Desktop             | No gate (user gesture)                                                                    | `{ url, applicant_notes?, applied?, extra_links? }`                                                                                                               | Read job; optional applied-on-arrival mark; autofill v2 extra_links parser hint; normalized URL dedup                                                                                                                                                           |
| `profile.get` / `profile.result`                                                            | Extension → Desktop             | `autofill_enabled`                                                                        | `{ fullName, email, phone, location, linkedin, github, website }`                                                                                                 | Fetch-fresh Contact Profile; none persist in browser; gate enforced desktop-side                                                                                                                                                                                |
| `answers.save` / `answers.result`                                                           | Extension → Desktop             | `autofill_enabled`                                                                        | `{ applicationId, answers: [{ question, answer }] }`                                                                                                              | Capture filled form answers; merge/dedup by question text per Application; no global answer pool                                                                                                                                                                |
| `answers.suggest` / `answers.suggest.result`                                                | Extension → Desktop             | `autofill_enabled`                                                                        | `{ applicationId } → { suggestions: [{ question, answer, sourceCompany?, sourceTitle?, score, salary }] }`                                                        | Local token-Jaccard replay; read-only lookup against stored answers for this Application                                                                                                                                                                        |
| `match.live` / `match.live.result`                                                          | Extension → Desktop             | No gate                                                                                   | `{ url } → { matchScore? }`                                                                                                                                       | Ad-hoc keyword-only fit scoring (no import); cache key matches import normalization; score is confidence `[0..1]`                                                                                                                                               |
| `applied.check` / `applied.result`                                                          | Extension → Desktop             | No gate                                                                                   | `{ url } → { found, applicationId?, status?, title?, appliedAt? }`                                                                                                | Read-only badge data; no Application writes; on-the-fly Application lookup (no dedup); `status` is one of {saved, applied, rejected, accepted, cancelled}                                                                                                       |
| `status.update` / `status.result`                                                           | Extension → Desktop             | No gate (user gesture)                                                                    | `{ url, to: "applied" } → { ok, applicationId, status, error? }`                                                                                                  | **Narrowest WRITE verb**: `saved → applied` ONLY transition; exact normalized-url match; atomic CAS (compare-and-set in store); no other transition allowed                                                                                                     |
| `answer.assist` / `assist.chunk` / `assist.done` / `assist.cancel` / `answer.assist.result` | Extension → Desktop             | AI-assist opt-in via `BridgeState` ([ADR 0011](../adr/0011-extension-ai-assist-optin.md)) | Request/streaming: assist `{ question, url?, search_web? }`, chunk `{ delta }`, cancel (no payload), done (no payload), result `{ ok, question, draft, sourced }` | Billable AI drafting; provider/model/base_url snapshotted at enable time in `AiAssistConfig`; per-connection per-reqId registration via `AssistStreamRegistry`; streaming chunks ordered per-reqId; `assist.cancel` marks for early-abort before compose starts |

**Wire-error discipline:** All verbs use fixed sentinel text (no dynamic/path/PII content) in error payloads — detailed context belongs in desktop log, not the wire. Errors never echo page-derived text.

## Status transition allowlist

The `status.update` verb is the **only** mutation the bridge permits on Applications. It enforces one hard rule: **`saved → applied` on an exact normalized-url match, and nothing else**. Every other combination is a refusal:

- The `to` field must be the literal string `"applied"` (re-validated in Rust independently of TS Zod).
- The row must exist for the exact normalized URL (no fuzzy match, no create-on-miss).
- The row's CURRENT status must be exactly `saved` (no re-applying an already-applied row, no transition out of any other stage).
- The write is atomic (compare-and-set in a single transaction): a concurrent Application-page mutation that changes status between the Rust handler's read and write is safely detected and refused, never silently overwritten.
- On success, the handler emits an `APPLICATIONS_CHANGED` event + Notification Center card, making the change visible without reopening the app.

This narrow gate keeps the bridge stateless (no complex transition rules) and makes the extension a **status reader** (via `applied.check`) + **status writer** (via `status.update`, one path only), not a full Application mutator.

## Import flow

Single unified import: `background.ts::runImport` always tries DOM capture first (`scripting.executeScript` → `content.js`), falls back to URL-only if capture is blocked (restricted pages). No user-visible mode selection — one **Import this job** button. The bridge side (`extension_bridge/mod.rs::handle_import`) acquires the shared `"scrape_url"` rate-limiter slot (same key/constants as `scrape_url` IPC: 30 req/60 s, 2 concurrent — see `limits/mod.rs` `SCRAPE_RATE_MAX` / `SCRAPE_CONCURRENCY_MAX`).

## Streaming transport (answer.assist)

The `answer.assist` verb streams AI-drafted answers back to the extension over the same socket in multiple frames, addressing a new need: long-running billable operations that should not block the popup UI or force the user to wait for a full round-trip before seeing the draft.

**Streaming frames per reqId (in order):**

1. `assist.chunk { delta }` — zero or more chunks, each containing a slice of the draft text. Multiple chunks for one answer concatenate in arrival order.
2. `assist.done` — terminal marker (no payload); signals the end of streaming for this reqId, whether successful or failed.
3. `answer.assist.result { ok, question, draft, sourced }` — final result frame (success or error). Carries the full draft + metadata, allowing the extension to validate/display.

**Per-connection registration (`AssistStreamRegistry`):**

To guard against cancellation-race bugs and billable-job leaks, every in-flight streaming request is tracked by a per-connection registry:

- **Before any pre-compose work** (such as a grounding query), `begin(reqId)` reserves the `reqId` as `Pending` — returns `true` if fresh, `false` if the reqId already has an entry (duplicate request, reject).
- **After grounding resolves but before starting the billable compose**, `register(reqId, jobId)` atomically moves `Pending → Running(jobId)`. If `cancel` arrived while `Pending`, `register` finds `CancelledEarly` instead, skips the billable compose, and returns `false` — caller aborts without charging budget.
- **On any cancel request** (user gesture, timeout), `cancel(reqId)` marks the entry for cancellation. If already `Running`, it cancels the job immediately. If still `Pending`, it marks `CancelledEarly` so that any subsequent `register` sees the marker and aborts.
- **On disconnect**, `cancel_all()` drains the registry: all `Running` jobs are cancelled (via `JobCanceller`), and all `Pending` + already-`CancelledEarly` entries are re-inserted as `CancelledEarly` (the critical guard: never drop a cancellation marker, or a re-registering request finds the entry vanished and starts a full billable generation).
- **At stream end**, `unregister(reqId)` forgets the entry.

**CancelledEarly invariant (critical):** A `CancelledEarly` marker must never be dropped during `cancel_all()` or `begin()` — if the marker is lost, the entry vanishes from the map, so a later `register()` call (from a retry, or from a race-condition path) finds no marker, thinks the reqId is fresh, and starts a full billable generation for a request the user already cancelled. The fix: `cancel_all()` exhaustively re-inserts **both** `Pending` and already-`CancelledEarly` entries as `CancelledEarly` when draining, so the marker persists until the client fully disconnects or gives up.

**Streaming chunks are ordered per-reqId:** All `assist.chunk`, `assist.done`, and `answer.assist.result` frames for one reqId are sent in order over the same socket/channel, maintaining the invariant that a popup UI can safely concatenate chunks and render a coherent draft (no out-of-order or duplicate chunks).

## Store policy

Chrome Web Store + Firefox AMO: MV3, no remote code, least-privilege permissions, single-purpose, honest metadata, privacy/data disclosure. Full pre-submission checklist in `.claude/skills/extension-standards/SKILL.md`.

## Agent count

The full fleet has 23 agents (21 domain agents + `cleanup` + `project-steward`). See `CLAUDE.md` routing table for the complete list.
