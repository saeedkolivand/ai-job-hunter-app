/**
 * Manifest source of truth (MV3) for both browser targets.
 *
 * The Vite build (`vite.config.mts`) selects a target via the `BROWSER` env and
 * emits the resolved object as `manifest.json` into that target's `dist/`. We
 * keep one typed base and apply only the per-browser delta so the two manifests
 * can never silently diverge.
 *
 * ── Pinned extension ids (MUST match the desktop allowlist) ──────────────────
 * The desktop bridge's origin allowlist lives in
 * `apps/desktop/src-tauri/src/extension_bridge/auth.rs::ALLOWED_EXTENSION_IDS`.
 * Pairing only works when the extension's runtime origin
 * (`chrome-extension://<id>` / `moz-extension://<id>`) is in that list.
 *
 * The Firefox AMO id below is an email-style AMO id tied to the aijobhunter.app
 * domain. Firefox runtime origins use a per-install `moz-extension://<uuid>`
 * (never the AMO id), so `auth.rs` admits Firefox by UUID shape, not a pinned id.
 * The published Chrome Web Store id IS pinned in
 * `auth.rs::ALLOWED_EXTENSION_IDS`; a locally-loaded build is admitted only via
 * the dev-origin override (`AJH_EXTENSION_DEV_ORIGINS`).
 */

import { version as VERSION } from '../package.json' with { type: 'json' };

export type BrowserTarget = 'chrome' | 'firefox';

/**
 * Firefox AMO extension id — an email-style AMO id tied to the aijobhunter.app
 * domain (the product owner owns the domain). Mirrors the Firefox entry in
 * `auth.rs::ALLOWED_EXTENSION_IDS` exactly. (The Chrome CWS id is still assigned
 * at publish — see below.)
 */
const FIREFOX_EXTENSION_ID = 'job-importer@aijobhunter.app';

/**
 * Loopback-only host permission. The background worker opens
 * `ws://127.0.0.1:<port>` to the desktop bridge; Firefox (and, defensively,
 * Chrome) require the connection target to be covered by a host permission.
 * Scoped to loopback ONLY — never any public/LAN host. See README "Permissions".
 */
const LOOPBACK_HOSTS = ['ws://127.0.0.1/*', 'http://127.0.0.1/*'];

/**
 * Permissions BOTH targets request. Kept as a named constant because the
 * per-target delta below is now non-empty (`sidePanel`, Chrome only) and the
 * pin in `manifest.test.ts` compares the two targets modulo exactly that
 * declared delta — a shared literal is what makes "one surface plus one
 * argued exception" readable rather than inferred from two spelled-out lists.
 */
const SHARED_PERMISSIONS = [
  'activeTab',
  'storage',
  'scripting',
  'nativeMessaging',
  'contextMenus',
] as const;

/**
 * Chrome's side-panel permission — the ONE declared per-target delta (ADR-044
 * decision 7). Firefox has no equivalent permission: it declares the
 * `sidebar_action` manifest key instead (see {@link firefoxManifest}), which
 * is why the both-targets pin asserts that key is present whenever this
 * permission is.
 */
const CHROME_ONLY_PERMISSIONS = ['sidePanel'] as const;

/** The Answer-tools panel document, shared by both targets' panel surfaces. */
const PANEL_PAGE = 'sidepanel.html';

/** Title shown on the panel/sidebar in both browsers' own chrome. */
const PANEL_TITLE = 'AI Job Hunter — Answer tools';

type ManifestRecord = Record<string, unknown>;

/** Fields shared by both targets. */
function baseManifest(): ManifestRecord {
  return {
    manifest_version: 3,
    name: 'AI Job Hunter — Job Importer',
    // ≤132 chars — Chrome Web Store caps the manifest `description`. Mentions
    // both capabilities (import + assisted autofill) honestly; the autofill
    // capability is opt-in/off-by-default (see the Settings toggle + README).
    description:
      'Import job postings into, and fill application forms from, your local AI Job Hunter desktop app over a private loopback connection.',
    version: VERSION,
    // activeTab → read the clicked tab's DOM on user action without broad host
    // access. storage → persist the pairing token. scripting → MV3 dynamic
    // inject for Scan mode (chrome.scripting.executeScript); host scope still
    // limited to the active tab by activeTab. nativeMessaging → `runtime.connectNative`
    // to the desktop host (`app.aijobhunter.bridge`), the HTTPS-Only-safe transport
    // that survives Firefox upgrading `ws://` to `wss://`; the loopback
    // `host_permissions` below stay for the `ws` fallback. contextMenus → two
    // entries (ADR-044 decision 2, amended): "Answer this with AI Job Hunter"
    // on selected text, and "Open answer panel" on a plain page right-click.
    // Either click is itself the user gesture that grants `activeTab` and
    // opens the answer panel — no standing page access comes with it.
    permissions: [...SHARED_PERMISSIONS],
    host_permissions: LOOPBACK_HOSTS,
    action: {
      default_popup: 'popup.html',
      default_title: 'Import this job into AI Job Hunter',
      default_icon: {
        '16': 'icons/icon-16.png',
        '32': 'icons/icon-32.png',
        '48': 'icons/icon-48.png',
        '128': 'icons/icon-128.png',
      },
    },
    icons: {
      '16': 'icons/icon-16.png',
      '32': 'icons/icon-32.png',
      '48': 'icons/icon-48.png',
      '128': 'icons/icon-128.png',
    },
    // No remotely-hosted code, no eval — everything is bundled. We deliberately
    // do NOT loosen content_security_policy.
  };
}

/** Chrome: MV3 service worker as an ES module. */
function chromeManifest(): ManifestRecord {
  return {
    ...baseManifest(),
    permissions: [...SHARED_PERMISSIONS, ...CHROME_ONLY_PERMISSIONS],
    background: {
      service_worker: 'background.js',
      type: 'module',
    },
    // The panel is opened from the popup's own click handler via
    // `chrome.sidePanel.open({ tabId })` and from the context-menu entry —
    // never via `openPanelOnActionClick`, which a declared `default_popup`
    // takes priority over anyway (ADR-044 decision 2).
    side_panel: { default_path: PANEL_PAGE },
  };
}

/**
 * Firefox: MV3 uses a non-persistent event page (`background.scripts`), not a
 * `service_worker`, and pins the add-on id via `browser_specific_settings`.
 * `strict_min_version` is `140.0` because that is the first release where Firefox
 * honors `browser_specific_settings.gecko.data_collection_permissions` (the AMO
 * data-consent key); this floor drops Firefox 115–139 / Android <142 users.
 */
function firefoxManifest(): ManifestRecord {
  return {
    ...baseManifest(),
    background: {
      scripts: ['background.js'],
      type: 'module',
    },
    // Firefox's sidebar is a MANIFEST KEY, not a permission — the counterpart
    // of Chrome's `sidePanel` permission + `side_panel` key above. It is
    // opened from a user gesture with `browser.sidebarAction.open()`.
    sidebar_action: {
      default_panel: PANEL_PAGE,
      default_title: PANEL_TITLE,
      default_icon: {
        '16': 'icons/icon-16.png',
        '32': 'icons/icon-32.png',
        '48': 'icons/icon-48.png',
        '128': 'icons/icon-128.png',
      },
    },
    browser_specific_settings: {
      gecko: {
        id: FIREFOX_EXTENSION_ID,
        strict_min_version: '140.0',
        // AMO data-collection declaration — stays ['none'] AND that is honest,
        // including with assisted autofill. The extension neither collects nor
        // transmits data to the developer or any third party: the contact profile
        // is the user's OWN data, fetched over loopback from their OWN paired
        // desktop app, held only for the one click, and written into the form on
        // the page the user chose to fill. Nothing is stored by the extension and
        // nothing leaves the device to a server. (Re-verify against the AMO
        // taxonomy before each submission; if the extension ever transmits profile
        // data off-device, declare personallyIdentifyingInfo/locationInfo here.)
        data_collection_permissions: { required: ['none'] },
      },
      // Android's data_collection_permissions support landed in 142; this is a
      // desktop-companion extension (pairs with the desktop app over loopback)
      // so it cannot function on Firefox for Android anyway — the higher Android
      // floor has no real-user impact.
      gecko_android: { strict_min_version: '142.0' },
    },
  };
}

/** Resolve the manifest object for a target. */
export function buildManifest(target: BrowserTarget): ManifestRecord {
  return target === 'firefox' ? firefoxManifest() : chromeManifest();
}
