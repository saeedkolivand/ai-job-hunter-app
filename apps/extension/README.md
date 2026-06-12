# AI Job Hunter — Browser Extension (`@ajh/extension`)

MV3 browser extension (Chrome + Firefox) that imports the job posting you are
viewing into the **AI Job Hunter desktop app** over a private, loopback-only
WebSocket. It is the browser half of Feature 2; the desktop half (the bridge
server, parser, and Applications store) already lives in
`apps/tauri/src-tauri/src/extension_bridge`.

> The wire protocol is owned by `@ajh/shared`
> (`packages/shared/src/ipc/extension-protocol.ts`) and imported here — it is
> never redefined in this package.

---

## Single-purpose statement (store listings)

**This extension does exactly one thing: it sends the job posting on the
current tab to the user's own AI Job Hunter desktop app running on the same
computer.** There is no account, no analytics, no remote backend. It is inert
unless the desktop app is running and the user has paired it.

---

## How it works

1. The desktop app runs a WebSocket server bound to `127.0.0.1` on the first
   free port in `47615..47620`.
2. The extension's background worker probes that exact port range to find it.
3. The user copies a **pairing token** from the app's
   **Settings → Browser extension** and pastes it into the extension's pairing
   screen. The token is stored in `chrome.storage.local` and sent on every
   frame; the desktop rejects any frame whose token mismatches.
4. From the popup the user picks a mode:
   - **Import via URL** — sends `{ url }`; the desktop fetches + parses it.
   - **Scan page** — injects a one-shot capture into the active tab and sends
     `{ url, html }` (the rendered, possibly authenticated DOM) so the desktop
     can parse pages its headless fetch can't reach.
   - An **"I already applied"** checkbox sets `applied: true`.
5. The desktop creates/updates a `saved` (or `applied`) Application, deduped by
   URL, and replies with `import.result`.

---

## Permissions — minimal & justified

| Permission                                                 | Why it is required                                                                                                                                                          |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `activeTab`                                                | Read the URL and (Scan mode) the DOM of the tab the user clicked on — **only** on that click, with no standing access to any site.                                          |
| `storage`                                                  | Persist the pairing token locally so the user pairs once.                                                                                                                   |
| `scripting`                                                | MV3 requires `scripting` to dynamically inject the Scan-mode capture via `chrome.scripting.executeScript`. Host scope stays limited to the active tab by `activeTab`.       |
| `host_permissions: ws://127.0.0.1/*`, `http://127.0.0.1/*` | **Loopback only.** The background worker opens `ws://127.0.0.1:<port>` to the desktop bridge. See the finding below — this is the narrowest entry that works cross-browser. |

We do **not** request broad host access (`<all_urls>`, `*://*/*`), no
`tabs` permission, no `webRequest`, and we do **not** loosen
`content_security_policy`. No remotely-hosted code and no `eval` — everything is
bundled at build time.

### Loopback host-permission finding (store-review sensitive)

A WebSocket to `ws://127.0.0.1` behaves differently per browser:

- **Firefox** requires the connection target to be covered by a
  `host_permissions` entry; without it the `ws://127.0.0.1` connection is
  blocked.
- **Chrome** is more permissive for loopback WebSockets from the background
  worker, but pinning the loopback host in `host_permissions` is the safe,
  explicit, review-defensible choice and does not broaden access beyond
  `127.0.0.1`.

**Decision:** include `host_permissions: ["ws://127.0.0.1/*", "http://127.0.0.1/*"]`
(loopback only) in both manifests. This is the narrowest entry that works on
both browsers; it grants **no** access to any public or LAN host.

---

## Privacy policy (store listings)

- The extension transmits page/job data **only** to the user's own AI Job Hunter
  desktop app over a loopback (`127.0.0.1`) connection on the same machine.
- **No data is ever sent to any remote or third-party server** by this
  extension. There is no telemetry, no analytics, no external API.
- The only stored value is the pairing token, kept in `chrome.storage.local`,
  used solely to authenticate to the local desktop app.
- Scan mode captures the current page's DOM **only when the user clicks "Scan
  page"**, and the captured HTML is sent only to the local app.

### Threat model

The extension finds the desktop bridge by probing `47615..47620` and sending
the pairing token to the **first loopback port that answers** (see the
`TODO(bridge)` comment in `src/lib/bridge.ts::probeRange`).

- **Risk (accepted for v1):** a malicious **same-account local process** that
  squats one of those ports **before** the desktop app binds it could accept the
  connection and harvest the pairing token on the user's first import.
- **Preconditions:** this requires an attacker that is **already running on the
  same machine under the same OS user account**. It is not reachable from the
  network or from any website — the connection target is loopback-only
  (`127.0.0.1`) and the host permission grants no public/LAN access.
- **Bounded impact:** the token only authorizes **local job-import** to the
  desktop bridge, whose fetch path is **SSRF-guarded**, and the token is
  **rotatable** from the app's Settings (re-pairing invalidates the old one).
  There is no remote backend or account to compromise.
- **Future fix:** add a server→client **challenge HMAC** over a nonce the
  desktop app **shows in-app** at pair time (so only the genuine app, which
  knows the nonce, can complete the handshake), or adopt the **native-messaging
  fallback** described below — a native messaging host **cannot be
  port-squatted**, since the browser launches it by registered name rather than
  by connecting to a listening port.

---

## Reviewer test notes

**The extension is non-functional without the AI Job Hunter desktop app** — this
is expected. A reviewer testing it standalone will see, by design:

- On open with the app **not running**: an _"AI Job Hunter isn't running"_
  empty state with a **Retry** button. No errors, no broken UI.
- With the app **running but unpaired**: a **pairing screen** asking for the
  64-character token from the app's Settings.
- With the app **running and paired**: the import view (two buttons + the
  "I already applied" checkbox).

To exercise the full path, install the desktop app from the project release,
open it, copy the token from **Settings → Browser extension**, paste it into the
extension, then open any job posting and click **Import via URL** or
**Scan page**.

---

## Build & load

```bash
# from the repo root
pnpm -F @ajh/extension build        # builds dist/chrome and dist/firefox
pnpm -F @ajh/extension build:chrome # Chrome only → apps/extension/dist/chrome
pnpm -F @ajh/extension build:firefox# Firefox only → apps/extension/dist/firefox
pnpm -F @ajh/extension typecheck
pnpm -F @ajh/extension lint
```

- **Chrome:** `chrome://extensions` → enable Developer mode → "Load unpacked" →
  select `apps/extension/dist/chrome`.
- **Firefox:** `about:debugging#/runtime/this-firefox` → "Load Temporary
  Add-on" → select `apps/extension/dist/firefox/manifest.json`.

### Build tooling

Plain **Vite 8** multi-entry (no `@crxjs`): `background.ts` and `content.ts`
build as standalone ES entries, `popup/popup.html` as an HTML entry, and a small
in-config plugin emits the per-browser `manifest.json` and copies the icons.
Chosen for determinism and to avoid the MV3 service-worker HMR fragility of
heavier plugins. Output is fully bundled — no remote chunks.

### Chrome vs Firefox manifests

One typed source (`src/manifest.ts`) with a per-target delta, selected by the
`BROWSER` env at build time:

- **Chrome** → `background.service_worker` + `type: module`.
- **Firefox** → `background.scripts` (MV3 non-persistent event page),
  `browser_specific_settings.gecko.id` + `strict_min_version: "115.0"`.

The `build` script runs both and writes `dist/chrome` and `dist/firefox`.

---

## AMO source-build note (Firefox)

AMO requires reviewable source for a bundled add-on. To reproduce the submitted
`dist/firefox` artifact from this repo:

```bash
pnpm install --frozen-lockfile
pnpm -F @ajh/shared build
pnpm -F @ajh/extension build:firefox
# artifact: apps/extension/dist/firefox
```

The build is deterministic: it bundles only this repo's source plus the pinned
dependencies in `package.json`; the manifest is generated from `src/manifest.ts`
with no network access. The output is not minified to a single line, so the
emitted JS stays readable for review.

---

## Pinned extension IDs — `TODO(bridge)` before store submission

The desktop bridge's origin allowlist
(`apps/tauri/src-tauri/src/extension_bridge/auth.rs::ALLOWED_EXTENSION_IDS`)
currently carries **placeholders**, matched here:

- Chrome: `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` (the real CWS id is assigned at
  publish and cannot be forced from the manifest — it only needs to be set in
  `auth.rs`).
- Firefox: `00000000-0000-0000-0000-000000000000`
  (`browser_specific_settings.gecko.id` here **and** in `auth.rs`).

**Before store submission, replace BOTH sides** with the real published IDs.
Until then, only the desktop dev-origin override
(`AJH_EXTENSION_DEV_ORIGINS`) admits a locally-loaded build.

---

## Native-messaging fallback

This v1 uses a loopback WebSocket. If a future store policy (or a hardened OS
network config) blocks loopback WS from an extension context, the fallback is
**native messaging**: register a native messaging host with the desktop app and
swap `BridgeClient`'s transport from `WebSocket` to
`browser.runtime.connectNative`. The wire envelope (`@ajh/shared`) and the
desktop handlers stay identical — only the transport changes. Tracked as a
`TODO(bridge)` follow-up; not implemented in v1.
