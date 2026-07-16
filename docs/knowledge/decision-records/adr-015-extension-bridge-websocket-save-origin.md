# ADR-015: Browser extension imports via local WebSocket bridge

Last updated: 2026-07-16

**Status:** Accepted

## Context

Feature 2 adds a browser extension ("Save this job" button on job boards) that must persist jobs into the desktop app's Application store without opening a dialog or requiring a separate UI flow. The extension runs in the user's browser, the app runs in the Tauri process — they are separate processes on the same machine, so they need a private IPC channel.

## Decision

The extension imports jobs into the app via a **local WebSocket server** running inside the Tauri Rust process (`apps/desktop/src-tauri/src/extension_bridge/`). The browser extension connects to `ws://127.0.0.1:<port>` and sends a frame matching the shared protocol defined once in `packages/shared/src/ipc/extension-protocol.ts`. The protocol uses Rust parity tests to keep the Rust message-type constants (`extension_bridge/mod.rs`) synchronized with the TS enum.

The app treats the browser as a **Save** Application origin (`ApplicationOrigin::Saved` → `saved` status, or `applied` if the request flags it). Import reuses `ApplicationStore::upsert_for_origin` to deduplicate by normalized URL, mirroring the save path — the first save creates the Application; a later save from the extension merges onto the existing row.

### Security model (five layers)

1. **Loopback bind only** — listener binds `127.0.0.1`; no LAN or remote access.
2. **Origin allowlist (defense-in-depth, not the primary boundary)** — the WS handshake's `Origin` header is validated in-callback (before socket upgrade) against `chrome-extension://<id>` and `moz-extension://<uuid>` patterns. **Chrome**: pinned to the stable Chrome Web Store id in `auth::ALLOWED_EXTENSION_IDS` (PR #379). **Firefox**: accepts any well-formed UUID shape (8-4-4-4-12 lowercase hex) via `auth::is_extension_uuid`, because Firefox assigns every install a random per-install internal UUID (anti-fingerprinting), never the AMO gecko id. A `platform::config::extension_dev_origins` env override admits local extensions during development. **Note:** the origin allowlist is defense-in-depth; the per-frame token (layer 3) is the authoritative auth boundary.
3. **Per-frame token** — every message envelope carries the paired secret; a mismatch closes the socket. The token is generated on first run, persisted to the app data dir, and rotatable via `extension_bridge_regenerate_token` command. This is the real auth boundary, even over loopback.
4. **Frame size cap** — messages over `MAX_FRAME_BYTES` (2 MB) are rejected at the protocol layer; the oversized frame closes the socket.
5. **URL/SSRF guard** — the imported `url` field is normalized (http/https only via `normalize_job_url`), then validated by `auth::is_safe_import_url` against loopback/private/link-local/`*.local` hosts. The actual fetch is additionally guarded by `net::http::get_guarded`, which resolves the hostname, validates the IP via `net::ssrf::is_safe_ip` (rejects RFC-1918, loopback, link-local, CGNAT, ULA, unspecified, broadcast, multicast), and pins the connection to that IP — closing the DNS-rebinding TOCTOU window.

### Import modes

- **URL mode** — extension sends `{ url }`, no `html`. Desktop resolves the URL via `scraping::scrape_url::resolve`, which is the standard headless scraper path.
- **Scan mode** — extension sends `{ url, html }` where `html` is the authenticated DOM from the user's browser (headless fetch cannot reach auth-walled boards). Desktop parses via `scraping::scrape_url::parse_from_html` (fetch-free parser, reuses the same selectors).

### Layering

The bridge is an **L3 shell module** (like `commands`, `tray`, `updater`): it holds an `AppHandle`, emits Tauri events (`applications:changed`), and reaches down into L1 (`applications`, `postings`, `scraping`). Server startup is fire-and-forget — a bind failure logs and disables the bridge but never blocks app boot.

## Considered options

1. **Local WebSocket server with origin + token gates (chosen).** Persistent bidirectional channel; allows future PUSH from app to extension (live match score, "already applied" badge, guided autofill) without re-architecting. Per-frame token + IP-pinned SSRF guard scale to handle high-volume scraping safely.

2. **One-shot HTTP POST to `127.0.0.1` + token in body.** Simpler server, but unidirectional; bidirectional features (live match, autofill hints, batch apply) would require polling or re-architecting the transport. Chrome Web Store reviewers are suspicious of loopback HTTP-only (no TLS); WS is less scrutinized because browsers treat it as a distinct channel.

3. **Native messaging (`chrome.runtime.connectNative` / `nativeMessaging.postRequest`).** Symmetric, browser-approved, no loopback socket. But review gates are higher (per-store native-messaging policy, list of approved extensions), release is slower (both stores), and fallback is harder (if a store rejects the native-messaging permission, the feature is dead). Kept as the **documented fallback** if a store blocks loopback WS.

## Consequences

- **Publication:** Chrome Web Store id pinned in `auth::ALLOWED_EXTENSION_IDS` (PR #379). Firefox validates by UUID shape (no separate entry required). Hosted privacy policy required; graceful "app not running" empty state handles connection failures.
- **Token management:** the pairing token is copied by the user from app Settings and entered/saved by the extension (the extension stores it, not the user). Rotation via `extension_bridge_regenerate_token` command; factory reset clears the token (Resettable trait).
- **v2 auth upgrade (shipped):** the pairing token is now authenticated via mutual HMAC-SHA256 challenge-response handshake (ADR-0010, PR #627), closing the v1 risk of early token harvest. The handshake is server-initiated (send random nonce, extension signs it); post-auth, frames carry no token.
- **Renderer integration:** `useMenuIntents` hook in renderer services subscribes to `applications:changed` event; the jobs/applications view live-updates on successful import.

## Related ADRs

- **ADR-001** (Rust-first business logic) — the bridge Rust implementation is the authoritative import path; the TS protocol is a thin mirror.
- **ADR-003** (Centralized platform/net error layers) — `net::ssrf` and `net::http::get_guarded` are shared guards; the bridge reuses them.
- **ADR-007** (Applications aggregate) — the browser is a Save origin; dedup and merge via `ApplicationStore::upsert_for_origin`.
