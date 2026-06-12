# Browser Extension — AI Job Hunter

<p align="center">
  <strong>One-click job import from your browser to the desktop app.</strong>
</p>

<p align="center">
  <a href="https://img.shields.io/badge/MV3-Chrome%20%7C%20Firefox-24C8DB"><img alt="MV3" src="https://img.shields.io/badge/MV3-Chrome%20%7C%20Firefox-24C8DB"></a>
  <a href="https://img.shields.io/badge/loopback-only-2ea44f"><img alt="Loopback only" src="https://img.shields.io/badge/loopback-only-2ea44f"></a>
  <a href="https://img.shields.io/badge/unpublished-dev-orange"><img alt="Unpublished" src="https://img.shields.io/badge/unpublished-dev-orange"></a>
</p>

This is the browser half of the **AI Job Hunter** job-import feature. An MV3 extension (Chrome + Firefox) that captures the job posting on the current tab and sends it to the **desktop app** running on your machine, over a private loopback WebSocket. No account. No remote backend. It is inert unless the desktop app is running and you have paired it.

**What it does:** while browsing a job board, click the extension button → choose **Import via URL** (extension sends the job URL, desktop fetches + parses it) or **Scan page** (extension captures the rendered DOM, desktop parses it) → the job appears in your **AI Job Hunter** saved applications, tagged **New**. Tick **"I already applied"** to mark it applied instead.

The desktop half (bridge server, parser, Applications store) lives in `apps/tauri/src-tauri/src/extension_bridge/`. The wire protocol is shared: `packages/shared/src/ipc/extension-protocol.ts`.

---

## How it works

```
1. Desktop app starts → runs loopback WebSocket server on 127.0.0.1:<port>
   ↓
2. Extension's background worker probes port range 47615..47620
   ↓
3. You copy the pairing token from app Settings → paste into extension
   ↓
4. Open any job board → click extension button → pick import mode
   ↓
5. Desktop receives frame → fetches/parses job → creates saved Application
```

---

## Quick Start

```bash
# Build the extension
pnpm -F @ajh/extension build

# Load unpacked:
# Chrome:  chrome://extensions → Developer mode → Load unpacked → dist/chrome
# Firefox: about:debugging → Load Temporary Add-on → dist/firefox/manifest.json
```

To actually **use** it, pair with the desktop app:

1. Open the desktop app → go to **Settings → Browser extension**
2. Copy the pairing token
3. Open the extension popup → paste the token
4. You'll see **Import via URL** and **Scan page** buttons
5. Open a job board and click one of them

For **local development**, see the section below — the unpacked extension gets a dev id that doesn't match the published allowlist, so you need to set an env var when starting the desktop app.

---

## Local Development & Testing

When you load an unpacked extension (Chrome or Firefox), the browser assigns it a fresh, machine-specific id that **does not** match the published extension ids in the desktop app's allowlist. The app will reject the pairing handshake unless you tell it to trust your dev id.

### Setup

1. **Build the extension:**

   ```bash
   pnpm -F @ajh/extension build
   ```

2. **Load unpacked:**
   - **Chrome:** `chrome://extensions` → toggle **Developer mode** → click **Load unpacked** → select `apps/extension/dist/chrome` → note the extension id (e.g. `abcdef123...`)
   - **Firefox:** `about:debugging#/runtime/this-firefox` → click **Load Temporary Add-on** → select `apps/extension/dist/firefox/manifest.json` → note the uuid from `about:debugging` (e.g. `12345678-1234-1234-1234-123456789abc`)

3. **Start the desktop app with the dev-origin override:**

   **PowerShell:**

   ```powershell
   $env:AJH_EXTENSION_DEV_ORIGINS = "chrome-extension://abcdef123..."
   pnpm dev
   ```

   **Bash / macOS / Linux:**

   ```bash
   AJH_EXTENSION_DEV_ORIGINS="chrome-extension://abcdef123..." pnpm dev
   ```

   For **Firefox**, use the uuid:

   ```bash
   AJH_EXTENSION_DEV_ORIGINS="moz-extension://12345678-1234-1234-1234-123456789abc" pnpm dev
   ```

4. **Pair the extension:**
   - Desktop app is now running with your dev extension id allowed
   - Extension popup → paste the pairing token from app Settings
   - You'll see the import buttons

5. **Test import modes:**
   - **Import via URL:** open any job posting → click **Import via URL** → job appears in Saved
   - **Scan page:** click **Scan page** → extension captures the DOM → desktop parses it (useful for authenticated pages)
   - Tick **"I already applied"** to mark as applied instead of saved

### Tips

- Chrome dev id changes if you remove and re-load the unpacked extension — use `chrome://extensions` to get the new id each time
- Firefox uuid stays the same across reloads (check `about:debugging` to confirm)
- The pairing token can be regenerated any time from app Settings → Browser extension

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

## Build & Development Commands

```bash
# from the repo root
pnpm -F @ajh/extension build        # builds both Chrome and Firefox → dist/
pnpm -F @ajh/extension build:chrome # Chrome only → dist/chrome
pnpm -F @ajh/extension build:firefox# Firefox only → dist/firefox
pnpm -F @ajh/extension dev          # watch mode (Chrome)
pnpm -F @ajh/extension typecheck    # TypeScript check
pnpm -F @ajh/extension lint         # ESLint
pnpm -F @ajh/extension test         # Vitest suite
```

### Build Tooling

Plain **Vite 8** multi-entry (no `@crxjs` or heavy framework plugins): `background.ts` and `content.ts` as standalone ES entries, `popup/popup.html` as an HTML entry, and a small in-config plugin generates the per-browser `manifest.json` and copies icons. Chosen for determinism and to sidestep MV3 service-worker HMR fragility. Output is fully bundled — no remote chunks, no external dependencies loaded at runtime.

Browser API compatibility: **`@wxt-dev/browser`** (MV3-native polyfill) — not `webextension-polyfill`, which was dropped as obsolete for Manifest v3.

### Chrome vs Firefox Manifests

One typed TypeScript source (`src/manifest.ts`) with a per-target delta, selected by the `BROWSER` environment variable at build time:

- **Chrome:** `background.service_worker` + `type: module`
- **Firefox:** `background.scripts` (MV3 event page), `browser_specific_settings.gecko.id`, `strict_min_version: "115.0"`

The `build` script runs both builders in sequence and writes `dist/chrome` and `dist/firefox`.

---

## CI & Release

The extension has its own CI path, path-filtered to only run when extension files change:

```bash
turbo run typecheck test build --filter=@ajh/extension
```

On release, the **Release workflow** includes a manual `package-extension` job (`workflow_dispatch`) that:

- Builds both Chrome and Firefox distributions
- Packages them as zips with source-build reproducibility info
- Uploads artifacts for store submission (when the extension is published)

Release frequency: the extension version tracks the app version via `pnpm sync:version` (currently unpublished, so no store cadence).

---

## Firefox AMO Source Build

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

## Native Messaging Fallback (Future)

This v1 uses a loopback WebSocket. If a store policy or hardened OS network config blocks loopback WS from an extension, the **fallback is native messaging**: register a native messaging host with the desktop app and swap `BridgeClient`'s transport from `WebSocket` to `browser.runtime.connectNative` (via `@wxt-dev/browser`).

The wire protocol envelope (`@ajh/shared`) and desktop handlers stay identical — **only the transport changes**. Native messaging is browser-approved, cannot be port-squatted, and works across permission policies. Tracked as a `TODO(bridge)` follow-up; not implemented in v1.
