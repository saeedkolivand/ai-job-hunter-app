/**
 * Settings page controller (PR0 §5) — a full browser tab (`options_ui`,
 * `open_in_tab: true`), reachable from the popup's "?" menu and the side
 * panel's gear button (`browser.runtime.openOptionsPage()`).
 *
 * Connection & pairing reuses `connection-status.ts` unchanged (the SAME
 * pill/pair-view module the popup and panel mount) — this page adds only its
 * own "Unpair this device" and "Open app settings →" controls, since the
 * popup's own unpair lives inside its "?" menu instead.
 *
 * "What the extension may do" renders THREE live toggles (PR1, R7 of the
 * redesign record — resolved in favor of toggling from the extension, not
 * read-only mirrors): `settings.get`/`settings.set` cover `autofill` /
 * `aiAssist` / `autotrack` today; the fourth switch, `saveAnswersOnSubmit`,
 * lands in PR4. A toggle click is OPTIMISTIC (flips immediately) and rolls
 * back on a refusal or a failed request — the desktop is still the source
 * of truth and re-enforces every gate at use time regardless of what this
 * page shows; every change made from here also raises a Notification
 * Center entry in the app.
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionSettingsKey, ExtensionSettingsValues } from '@ajh/shared';

import { mountConnectionStatus, PAIRING_DEEP_LINK } from '../connection-status/connection-status';
import {
  type DefaultPanelTab,
  getDefaultPanelTab,
  getShowFitBadge,
  getStampResultsPages,
  setDefaultPanelTab,
  setShowFitBadge,
  setStampResultsPages,
} from '../lib/appearance';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { forgetHost, getRememberedHosts } from '../lib/site-memory';
import { bootTheme, getTheme, setTheme, type Theme } from '../lib/theme';

import '../popup/popup.css';

void bootTheme();

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

const els = {
  connectionPillHost: byId<HTMLDivElement>('connection-pill-host'),
  connectionViewsHost: byId<HTMLDivElement>('connection-views-host'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  btnOpenAppSettings: byId<HTMLButtonElement>('btn-open-app-settings'),
  sitesList: byId<HTMLDivElement>('sites-list'),
  permissionsList: byId<HTMLDivElement>('permissions-list'),
  themeSeg: byId<HTMLDivElement>('theme-seg'),
  defaultTabSeg: byId<HTMLDivElement>('default-tab-seg'),
  toggleFitBadge: byId<HTMLButtonElement>('toggle-fit-badge'),
  toggleStampResults: byId<HTMLButtonElement>('toggle-stamp-results'),
  shortcutsList: byId<HTMLDivElement>('shortcuts-list'),
  aboutVersion: byId<HTMLParagraphElement>('about-version'),
  linkPrivacy: byId<HTMLAnchorElement>('link-privacy'),
  linkHelp: byId<HTMLAnchorElement>('link-help'),
  linkSource: byId<HTMLAnchorElement>('link-source'),
  linkReport: byId<HTMLAnchorElement>('link-report'),
};

async function send(req: PopupRequest): Promise<PopupResponse> {
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

async function openDeepLink(url: string): Promise<void> {
  try {
    await browser.tabs.create({ url });
  } catch {
    // Best-effort — same discipline as connection-status.ts's own deep links.
  }
}

// ── Connection & pairing ─────────────────────────────────────────────────

mountConnectionStatus(els.connectionPillHost, els.connectionViewsHost, {
  send,
  onStatus: (status) => {
    els.btnUnpair.hidden = !status.hasToken;
  },
  // A disconnected→connected transition (e.g. the desktop app was just
  // launched) means every switch may have changed since the last (possibly
  // "Unknown until connected") answer — re-fetch rather than leaving the
  // toggles stale for the rest of this page's lifetime.
  onConnected: () => void runSettingsGet(),
}).start();

els.btnUnpair.addEventListener('click', () => {
  void send({ kind: 'clearToken' });
});
els.btnOpenAppSettings.addEventListener('click', () => void openDeepLink(PAIRING_DEEP_LINK));

// ── Sites you've approved ────────────────────────────────────────────────

/** Exported for `options.test.ts` — no click/gesture re-drives this render
 *  other than a Forget click, which already needs an existing row to fire. */
export async function renderSites(): Promise<void> {
  const hosts = await getRememberedHosts();
  els.sitesList.replaceChildren();
  if (hosts.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'hint';
    empty.textContent =
      "You haven't approved any sites yet — they're added from the first Fill" + ' confirmation.';
    els.sitesList.append(empty);
    return;
  }
  for (const host of hosts) {
    const row = document.createElement('div');
    row.className = 'set-row';
    const copy = document.createElement('div');
    copy.className = 'set-row-copy';
    const title = document.createElement('p');
    title.className = 'set-title';
    title.textContent = host;
    copy.append(title);
    const forget = document.createElement('button');
    forget.className = 'btn btn--small btn--quiet';
    forget.type = 'button';
    forget.textContent = 'Forget';
    forget.addEventListener('click', () => {
      void forgetHost(host).then(renderSites);
    });
    row.append(copy, forget);
    els.sitesList.append(row);
  }
}
void renderSites();

// ── What the extension may do (LIVE toggles, PR1, R7) ────────────────────

interface PermissionRow {
  key: ExtensionSettingsKey;
  title: string;
  desc: string;
}

const PERMISSION_ROWS: readonly PermissionRow[] = [
  {
    key: 'autofill',
    title: 'Assisted autofill',
    desc: 'Fill forms on this page with your saved contact details, on request.',
  },
  {
    key: 'aiAssist',
    title: 'AI-answer-assist',
    desc: 'Draft and rewrite answers to application questions.',
  },
  {
    key: 'autotrack',
    title: 'Auto-track applied status',
    desc: 'Mark a job Applied automatically when its form submits.',
  },
];

/** The last known switch values, or `null` before the first `settings.get`
 *  answers (or after a disconnect) — a toggle stays disabled while this is
 *  `null`, since there is nothing to optimistically flip. */
let currentSettings: ExtensionSettingsValues | null = null;

function renderPermissions(settings: ExtensionSettingsValues | null): void {
  currentSettings = settings;
  els.permissionsList.replaceChildren();
  for (const row of PERMISSION_ROWS) {
    const el = document.createElement('div');
    el.className = 'set-row';
    const copy = document.createElement('div');
    copy.className = 'set-row-copy';
    const titleId = `perm-title-${row.key}`;
    const title = document.createElement('p');
    title.id = titleId;
    title.className = 'set-title';
    title.textContent = row.title;
    const desc = document.createElement('p');
    desc.className = 'set-desc';
    desc.textContent = settings ? row.desc : `${row.desc} (Unknown until connected.)`;
    copy.append(title, desc);

    const known = settings ? settings[row.key] : null;
    const toggle = document.createElement('button');
    toggle.className = 'toggle';
    toggle.type = 'button';
    toggle.setAttribute('role', 'switch');
    toggle.setAttribute('aria-labelledby', titleId);
    toggle.classList.toggle('on', known === true);
    toggle.setAttribute('aria-checked', String(known === true));
    toggle.disabled = known === null;
    toggle.addEventListener('click', () => void toggleSetting(row.key, toggle));

    el.append(copy, toggle);
    els.permissionsList.append(el);
  }
}

/** True while a `settings.set` round trip is in flight. Guards against a
 *  rapid double-click on the same switch, or a second switch clicked before
 *  the first reply — both would otherwise compute `next` from the same
 *  stale `currentSettings` snapshot and fire overlapping requests. */
let settingsRequestInFlight = false;

/**
 * Flip one switch, optimistically. `btn`'s visual state changes immediately
 * (so the click feels instant); a well-formed desktop refusal or a failed
 * request rolls the WHOLE row set back to the values from before the click
 * (simplest correct fix — a single-key rollback would drift from the
 * desktop's own reply on `ok:true`, which already carries the FULL
 * settings object for exactly this reason).
 */
async function toggleSetting(key: ExtensionSettingsKey, btn: HTMLButtonElement): Promise<void> {
  const prev = currentSettings;
  // Disabled while unknown, or while a previous toggle's request is still
  // in flight — defensive, a click shouldn't fire here either way.
  if (!prev || settingsRequestInFlight) return;
  const next = !prev[key];
  btn.classList.toggle('on', next);
  btn.setAttribute('aria-checked', String(next));

  settingsRequestInFlight = true;
  btn.setAttribute('aria-busy', 'true');
  for (const toggle of els.permissionsList.querySelectorAll<HTMLButtonElement>('.toggle')) {
    toggle.disabled = true;
  }

  try {
    const res = await send({ kind: 'settingsSet', key, enabled: next });
    if (res.ok && res.kind === 'settingsSet' && res.result.ok) {
      renderPermissions(res.result.settings);
      return;
    }
  } catch {
    // fall through to rollback — same discipline as a failed request below.
  } finally {
    // `renderPermissions` below (or above, on success) rebuilds the row set
    // from scratch — including each toggle's `disabled` state — so there is
    // nothing to manually re-enable here, only the guard to release.
    settingsRequestInFlight = false;
  }
  renderPermissions(prev);
}

/** Fetch `settings.get` and render the three live toggles — run once at
 *  load, and again on every disconnected→connected transition (the
 *  `onConnected` dep above), since the values live on the desktop side and
 *  can change between connections. */
async function runSettingsGet(): Promise<void> {
  try {
    const res = await send({ kind: 'settingsGet' });
    if (res.ok && res.kind === 'settingsGet' && res.result.ok) {
      renderPermissions(res.result.settings);
      return;
    }
  } catch {
    // fall through to the unknown state — same discipline as this page's
    // other fire-and-forget checks.
  }
  renderPermissions(null);
}

renderPermissions(null);
void runSettingsGet();

// ── Appearance ────────────────────────────────────────────────────────────

function setSegActive(seg: HTMLElement, attr: string, value: string): void {
  for (const btn of seg.querySelectorAll<HTMLButtonElement>('button')) {
    btn.classList.toggle('active', btn.dataset[attr] === value);
  }
}

els.themeSeg.querySelectorAll<HTMLButtonElement>('[data-theme-choice]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const choice = btn.dataset.themeChoice as Theme;
    void setTheme(choice);
    setSegActive(els.themeSeg, 'themeChoice', choice);
  });
});

els.defaultTabSeg.querySelectorAll<HTMLButtonElement>('[data-tab-choice]').forEach((btn) => {
  btn.addEventListener('click', () => {
    const choice = btn.dataset.tabChoice as DefaultPanelTab;
    void setDefaultPanelTab(choice);
    setSegActive(els.defaultTabSeg, 'tabChoice', choice);
  });
});

function wireToggle(
  btn: HTMLButtonElement,
  get: () => Promise<boolean>,
  set: (v: boolean) => Promise<void>
): void {
  function render(on: boolean): void {
    btn.classList.toggle('on', on);
    btn.setAttribute('aria-checked', String(on));
  }
  btn.addEventListener('click', () => {
    const next = !btn.classList.contains('on');
    render(next);
    void set(next);
  });
  void get().then(render);
}

wireToggle(els.toggleFitBadge, getShowFitBadge, setShowFitBadge);
wireToggle(els.toggleStampResults, getStampResultsPages, setStampResultsPages);

void getTheme().then((theme) => setSegActive(els.themeSeg, 'themeChoice', theme));
void getDefaultPanelTab().then((tab) => setSegActive(els.defaultTabSeg, 'tabChoice', tab));

// ── Shortcuts ─────────────────────────────────────────────────────────────

const isFirefox = typeof (browser as { sidebarAction?: unknown }).sidebarAction !== 'undefined';

interface ManifestCommand {
  description?: string;
  suggested_key?: { default?: string };
}

function renderShortcuts(): void {
  const manifest = browser.runtime.getManifest() as unknown as {
    commands?: Record<string, ManifestCommand>;
  };
  const entries = manifest.commands ? Object.entries(manifest.commands) : [];
  els.shortcutsList.replaceChildren();
  if (entries.length === 0) {
    const hint = document.createElement('p');
    hint.className = 'hint';
    hint.textContent = 'No shortcuts are declared yet.';
    const btn = document.createElement('button');
    btn.className = 'btn btn--quiet';
    btn.type = 'button';
    btn.textContent = "Open the browser's shortcut settings";
    btn.addEventListener('click', () => {
      void openDeepLink(isFirefox ? 'about:addons' : 'chrome://extensions/shortcuts');
    });
    els.shortcutsList.append(hint, btn);
    return;
  }
  for (const [, command] of entries) {
    const row = document.createElement('div');
    row.className = 'shortcut-row';
    const label = document.createElement('span');
    label.textContent = command.description ?? '';
    const kbd = document.createElement('span');
    kbd.className = 'kbd';
    kbd.textContent = command.suggested_key?.default ?? 'Unassigned';
    row.append(label, kbd);
    els.shortcutsList.append(row);
  }
}
renderShortcuts();

// ── Privacy & about ───────────────────────────────────────────────────────

els.aboutVersion.textContent = `Version ${browser.runtime.getManifest().version}`;
els.linkPrivacy.href = 'https://aijobhunter.app/privacy';
// Reuses the extension's own README (built-source disclosure, permissions
// table, troubleshooting) — there is no separate hosted help page for the
// extension to link to yet.
els.linkHelp.href =
  'https://github.com/saeedkolivand/ai-job-hunter-app/tree/main/apps/extension#readme';
els.linkSource.href = 'https://github.com/saeedkolivand/ai-job-hunter-app';
els.linkReport.href = 'https://github.com/saeedkolivand/ai-job-hunter-app/issues/new';
