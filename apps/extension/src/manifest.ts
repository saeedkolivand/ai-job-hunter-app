/**
 * Manifest source of truth (MV3) for both browser targets.
 *
 * The Vite build (`vite.config.ts`) selects a target via the `BROWSER` env and
 * emits the resolved object as `manifest.json` into that target's `dist/`. We
 * keep one typed base and apply only the per-browser delta so the two manifests
 * can never silently diverge.
 *
 * ── Pinned extension ids (MUST match the desktop allowlist) ──────────────────
 * The desktop bridge's origin allowlist lives in
 * `apps/tauri/src-tauri/src/extension_bridge/auth.rs::ALLOWED_EXTENSION_IDS`.
 * Pairing only works when the extension's runtime origin
 * (`chrome-extension://<id>` / `moz-extension://<id>`) is in that list.
 *
 * TODO(bridge): before store submission replace BOTH placeholders here AND the
 * matching constants in `auth.rs` with the real published Chrome Web Store id
 * and Firefox AMO id. Until then, only the dev-origin override
 * (`AJH_EXTENSION_DEV_ORIGINS` on the app side) admits a locally-loaded build.
 */

export type BrowserTarget = 'chrome' | 'firefox';

/** Firefox AMO extension id — PLACEHOLDER, mirrors auth.rs. TODO(bridge). */
export const FIREFOX_EXTENSION_ID = '00000000-0000-0000-0000-000000000000';

/**
 * Chrome Web Store id is assigned by the store at publish time and cannot be
 * forced from the manifest. The desktop allowlist carries the matching
 * placeholder (`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`). TODO(bridge): when the real
 * CWS id is known, set it in auth.rs (the manifest needs no id field for Chrome).
 */
export const CHROME_EXTENSION_ID_PLACEHOLDER = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

const VERSION = '0.1.0';

/**
 * Loopback-only host permission. The background worker opens
 * `ws://127.0.0.1:<port>` to the desktop bridge; Firefox (and, defensively,
 * Chrome) require the connection target to be covered by a host permission.
 * Scoped to loopback ONLY — never any public/LAN host. See README "Permissions".
 */
const LOOPBACK_HOSTS = ['ws://127.0.0.1/*', 'http://127.0.0.1/*'];

type ManifestRecord = Record<string, unknown>;

/** Fields shared by both targets. */
function baseManifest(): ManifestRecord {
  return {
    manifest_version: 3,
    name: 'AI Job Hunter — Job Importer',
    description:
      'Import the job posting you are viewing into your local AI Job Hunter desktop app over a private loopback connection.',
    version: VERSION,
    // activeTab → read the clicked tab's DOM on user action without broad host
    // access. storage → persist the pairing token. scripting → MV3 dynamic
    // inject for Scan mode (chrome.scripting.executeScript); host scope still
    // limited to the active tab by activeTab.
    permissions: ['activeTab', 'storage', 'scripting'],
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
    background: {
      service_worker: 'background.js',
      type: 'module',
    },
  };
}

/**
 * Firefox: MV3 uses a non-persistent event page (`background.scripts`), not a
 * `service_worker`, and pins the add-on id via `browser_specific_settings`.
 * `strict_min_version` 115 is the first ESR with stable MV3 event-page support.
 */
function firefoxManifest(): ManifestRecord {
  return {
    ...baseManifest(),
    background: {
      scripts: ['background.js'],
      type: 'module',
    },
    browser_specific_settings: {
      gecko: {
        id: FIREFOX_EXTENSION_ID,
        strict_min_version: '115.0',
      },
    },
  };
}

/** Resolve the manifest object for a target. */
export function buildManifest(target: BrowserTarget): ManifestRecord {
  return target === 'firefox' ? firefoxManifest() : chromeManifest();
}
