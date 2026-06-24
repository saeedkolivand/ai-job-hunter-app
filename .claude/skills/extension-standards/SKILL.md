---
name: extension-standards
description: Browser-extension + bridge standards — MV3 (Chrome + Firefox), least-privilege permissions, the loopback pairing-token auth model, the shared TS↔Rust wire protocol, and the Chrome Web Store + Firefox AMO store-policy + pre-submission checklist. Load for changes under apps/extension/**, extension_bridge/, and the extension protocol.
---

# Extension + bridge standards

Standards for the browser extension and the desktop⇄extension bridge. Load with `author-contract` (authors) / `token-efficiency` (reviewers).

## Architecture & paths

- **Extension** (`apps/extension/**`) — MV3, published on Chrome Web Store **and** Firefox AMO. Content scripts on supported job boards → import the user-selected posting into the desktop app.
- **Bridge** — the extension reaches the desktop app over a **loopback-only WebSocket** (`127.0.0.1`, a fixed port range) and **Chrome Native Messaging** (the native host relays frames verbatim). Server: `apps/tauri/src-tauri/src/extension_bridge/**` + `commands/extension_bridge.rs`.
- **Wire contract** — `packages/shared/src/ipc/extension-protocol-constants.ts` (envelope + `EXTENSION_MESSAGE_TYPES`), its zod schema `extension-protocol.ts`, and the Rust `msg` constants in the bridge. A Rust↔TS parity test pins the message-type strings together.

## Auth model (the security boundary)

- The **per-frame 256-bit pairing token** (loopback only) is what authenticates. The origin allowlist (Chrome id / Firefox UUID-shape / native-host sentinel) is **defense-in-depth only**.
- A connection is **authorized/connected ONLY after a frame passes the token check** — never on the WS handshake alone. On open the extension sends `{ type: "auth", token, reqId, payload: null }`; the desktop replies an `import.result` with **no `error`** (= authorized) or `{ error: "unauthorized" }` **and closes the socket**. A wrong token is never reported as connected. (This is the fix for the "any token looks authorized" bug — don't regress it.)
- Never echo/confirm the token back; never log it. Token file is owner-only (`0o600`) on unix.

## Protocol lockstep (don't let the two sides drift)

A new message type/field is added in the **same change** to: the TS `EXTENSION_MESSAGE_TYPES` constant, the TS zod `ExtensionMessageTypeSchema`, **and** the Rust `msg` module. The envelope shape (`type` / `token` / `reqId` / `payload`) stays identical on both sides. The parity + uniqueness tests must stay green.

**Browser-coverage lockstep** — when browser detection adds a browser (a new Chromium-family or Flatpak id), add its **native-messaging host manifest** entry in the **same change** (native + the per-app Flatpak dir); a detected browser with no manifest can't pair (#486 detected Vivaldi but never registered it). Grep the sibling manifest table for every id the detector can return.

**Rust + tests** — the bridge/detection Rust (`extension_bridge/**`, `platform/chrome/**`) and its tests also obey `rust-standards` + `testing-rules` (cfg-gated/cross-OS, bounded external processes, env-`#[serial]`, no host-coupled/`exec()`-in-test), not just `extension-standards`.

## Manifest V3 rules (both stores)

- **No remote code (absolute)** — all JS bundled in the package; no `eval`, no external `<script>`, no runtime code fetch. Fetching **data** (JSON) is fine; executing fetched **code** is not. ([CWS MV3](https://developer.chrome.com/docs/webstore/program-policies/mv3-requirements), [AMO policies](https://extensionworkshop.com/documentation/publish/add-on-policies/))
- **CSP** at/above the MV3 default (`script-src 'self' 'wasm-unsafe-eval'; object-src 'self'`).
- **Service worker** (Chrome) is event-driven; native messaging is a sanctioned reason it may outlive the idle limit. **Firefox** uses non-persistent background scripts/event pages (no persistent background in MV3).
- **`web_accessible_resources`** — expose the minimum, scoped to the specific board origins; prefer `use_dynamic_url` to reduce fingerprinting.
- **Firefox needs `browser_specific_settings.gecko.id`** for MV3 (Chrome ignores the key — keep one shared manifest).

## Permissions — least privilege

- Request only what an **existing** feature needs; **no future-proofing** (requesting a permission for an unbuilt feature is a rejection trigger). ([CWS permissions](https://developer.chrome.com/docs/webstore/program-policies/permissions))
- Scope `host_permissions` to the **exact supported board origins** — never `<all_urls>`. Prefer `activeTab` / optional permissions where they suffice.
- `nativeMessaging` only; native/WS calls run in the SW/extension pages, not content scripts.
- **Single purpose** (Chrome): one narrow, easily-understood purpose; don't bundle unrelated features.

## Store policy + pre-submission checklist

> **Date-sensitive (re-verify the two source pages before each submission):**
>
> - CWS Program Policies last updated **2025-05-22**; the 2025 wave added one-appeal-per-violation, a real-money-gambling ban, and single-purpose clarifications. ([2025 blog](https://developer.chrome.com/blog/cws-policy-updates-2025))
> - **Firefox: in H1 2026, Mozilla requires ALL extensions (incl. existing ones, on their next update) to adopt the built-in data-collection consent framework.** New extensions already required since **2025-11-03**. This extension is already published → treat as in-scope now. ([Firefox data consent](https://extensionworkshop.com/documentation/develop/firefox-builtin-data-consent/), [Mozilla blog](https://blog.mozilla.org/addons/2025/10/23/data-collection-consent-changes-for-new-firefox-extensions/))
> - **MV2 is end-of-life** — author/review against MV3 only.

Run this before any release (both stores):

- [ ] `host_permissions` scoped to exact board origins; no `<all_urls>`, no unused/future-proof perms; `nativeMessaging` declared.
- [ ] Zero remote code; CSP at/above MV3 default; `web_accessible_resources` minimal + origin-scoped.
- [ ] Data leaving the browser (bridge/native host) is **limited to the user-acted-on posting + the pairing handshake** — no persistent IDs, analytics, cookies, ad data; never shared with third parties. ([CWS limited use](https://developer.chrome.com/docs/webstore/program-policies/limited-use), [AMO](https://extensionworkshop.com/documentation/publish/add-on-policies/))
- [ ] **Chrome**: privacy-policy URL set; Privacy-practices/Data-usage disclosures + Limited-Use certification complete; in-product prominent disclosure + consent **before** collection; the pairing token + scraped posting both disclosed; HTTPS/secure transport. ([CWS privacy](https://developer.chrome.com/docs/webstore/program-policies/privacy), [dashboard privacy](https://developer.chrome.com/docs/webstore/cws-dashboard-privacy))
- [ ] **Firefox**: `browser_specific_settings.gecko.id` set; `gecko.data_collection_permissions` (`required`/`optional`, or `["none"]`) declared accurately (`technicalAndInteraction` can only be optional); consent UI present (own or built-in); privacy-policy link recommended.
- [ ] **Firefox source submission** (build is bundled/minified → required): full source + README build steps + lockfile; build reproduces and `diff`s clean against the package; no obfuscation. ([source submission](https://extensionworkshop.com/documentation/publish/source-code-submission/))
- [ ] Listing honest: real screenshots, icon, accurate single-purpose description, no keyword stuffing (<5/keyword).

## Common rejections

- **Chrome**: over-broad/unjustified `host_permissions`; any remote code; single-purpose violation; missing icon/screenshot/privacy-policy; incomplete data-usage tab; collecting data without in-UI disclosure+consent.
- **Firefox**: bundled code without reproducible source + lockfile; unnecessary permissions; undisclosed data transmission ("No Surprises"); leaking local/user data via native messaging; missing data-collection consent.
