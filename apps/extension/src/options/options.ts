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
 * "What the extension may do" renders THREE rows, not the design record's
 * four: `autofillCheck` is the only opt-in the extension↔background wire
 * (`lib/messages.ts`) can actually ask about today — AI-answer-assist and
 * auto-track have desktop-side settings (`packages/shared/…/extensionBridge.ts`)
 * but no matching extension-side check verb, and PR0 forbids adding one
 * ("no new bridge verbs"). Both read "Unknown until connected" rather than a
 * fabricated fourth row. // TODO(PR1 settings.set): turn all three into live
 * toggles — design record R7.
 */

import { browser } from '@wxt-dev/browser';

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
  // launched) means the ONE opt-in this page can actually ask about may have
  // changed since the last (possibly "Unknown until connected") answer —
  // re-run it rather than leaving the Assisted autofill row stale for the
  // rest of this page's lifetime.
  onConnected: () => void runAutofillCheck(),
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

// ── What the extension may do (read-only, PR0) ───────────────────────────

interface PermissionRow {
  title: string;
  desc: string;
  /** `true` once `autofillCheck` has answered; the other two rows never
   *  resolve in this PR (no check verb exists yet — see this file's doc). */
  checkable: boolean;
}

const PERMISSION_ROWS: readonly PermissionRow[] = [
  {
    title: 'Assisted autofill',
    desc: 'Fill forms on this page with your saved contact details, on request.',
    checkable: true,
  },
  {
    title: 'AI-answer-assist',
    desc: 'Draft and rewrite answers to application questions.',
    checkable: false,
  },
  {
    title: 'Auto-track applied status',
    desc: 'Mark a job Applied automatically when its form submits.',
    checkable: false,
  },
];

function renderPermissions(autofillEnabled: boolean | null): void {
  els.permissionsList.replaceChildren();
  for (const row of PERMISSION_ROWS) {
    const el = document.createElement('div');
    el.className = 'set-row';
    const copy = document.createElement('div');
    copy.className = 'set-row-copy';
    const title = document.createElement('p');
    title.className = 'set-title';
    title.textContent = row.title;
    const desc = document.createElement('p');
    desc.className = 'set-desc';
    desc.textContent = row.desc;
    copy.append(title, desc);

    const state = document.createElement('span');
    state.className = 'tag';
    const known = row.checkable ? autofillEnabled : null;
    if (known === null) {
      state.textContent = 'Unknown until connected';
    } else {
      state.textContent = known ? 'On' : 'Off';
      state.classList.add(known ? 'tag--ok' : 'tag--warn');
    }

    const caption = document.createElement('button');
    caption.className = 'link';
    caption.type = 'button';
    caption.textContent = 'Change in app →';
    caption.addEventListener('click', () => void openDeepLink(PAIRING_DEEP_LINK));

    el.append(copy, state, caption);
    els.permissionsList.append(el);
  }
}
/** Fetch `autofillCheck` and re-render the "Assisted autofill" row — run once
 *  at load, and again on every disconnected→connected transition (the
 *  `onConnected` dep above) since the opt-in it reports is set on the
 *  desktop side and can change between connections. */
async function runAutofillCheck(): Promise<void> {
  try {
    const res = await send({ kind: 'autofillCheck' });
    if (res.ok && res.kind === 'autofillCheck') renderPermissions(res.enabled);
  } catch {
    // Best-effort — same discipline as this page's other fire-and-forget checks.
  }
}

renderPermissions(null);
void runAutofillCheck();

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
