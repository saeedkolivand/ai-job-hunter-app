/**
 * Popup controller (plain TS — deliberately NOT the app's React stack).
 *
 * It is a thin view over the background worker: it sends typed
 * {@link PopupRequest}s, renders the {@link ConnectionStatus} the background
 * returns/pushes, and never talks to the desktop bridge directly. Store
 * reviewers test WITHOUT the desktop app, so every state must render an
 * explanation, never an error.
 */

import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus, ImportMode, PopupRequest, PopupResponse } from '../lib/messages';
import { looksLikeToken } from '../lib/storage';

import './popup.css';

// ── pure view-decision helpers (exported for unit tests) ─────────────────────

/**
 * Given a `getStatus` response, return the {@link ConnectionStatus} to render,
 * or `null` if the response signals the background is unreachable (use offline
 * fallback in that case).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveStatusResponse(
  res: PopupResponse,
  lastKnownHasToken: boolean
): ConnectionStatus {
  if (res.ok && res.kind === 'status') return res.status;
  // `!ok` or unexpected kind → offline fallback preserving last-known token.
  return { phase: 'app_not_running', port: null, hasToken: lastKnownHasToken };
}

/** Where an imported job lands in the desktop app — shown on success so the
 *  user knows where to look (the extension can't focus the native window). */
const IMPORT_LANDING_HINT = 'Open AI Job Hunter → Applications to view it.';

/**
 * Given an `import` response, return the message text and tone to display. On
 * success it names the imported job (when the desktop parsed a title) and points
 * the user at where it landed, instead of a bare "Imported".
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveImportResponse(res: PopupResponse): { text: string; tone: 'ok' | 'err' } {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'import') return { text: 'Unexpected response — please retry.', tone: 'err' };
  const { result } = res;
  if (result.error) return { text: result.error, tone: 'err' };
  const title = result.title?.trim();
  const lead = title ? `Imported “${title}”.` : 'Imported.';
  return { text: `${lead} ${IMPORT_LANDING_HINT}`, tone: 'ok' };
}

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

const els = {
  pill: byId<HTMLSpanElement>('status-pill'),
  views: {
    import: byId<HTMLElement>('view-import'),
    pair: byId<HTMLElement>('view-pair'),
    offline: byId<HTMLElement>('view-offline'),
    searching: byId<HTMLElement>('view-searching'),
  },
  btnUrl: byId<HTMLButtonElement>('btn-url'),
  btnScan: byId<HTMLButtonElement>('btn-scan'),
  chkApplied: byId<HTMLInputElement>('chk-applied'),
  importMsg: byId<HTMLParagraphElement>('import-msg'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  tokenInput: byId<HTMLInputElement>('token-input'),
  pairMsg: byId<HTMLParagraphElement>('pair-msg'),
  btnSaveToken: byId<HTMLButtonElement>('btn-save-token'),
  btnRetry: byId<HTMLButtonElement>('btn-retry'),
  btnOpenApp: byId<HTMLButtonElement>('btn-open-app'),
  btnOpenSettings: byId<HTMLButtonElement>('btn-open-settings'),
  btnHelp: byId<HTMLButtonElement>('btn-help'),
  helpPopover: byId<HTMLParagraphElement>('help-popover'),
};

/** Actionable label for the pairing button; restored after a failed/cleared pair. */
const PAIR_LABEL = 'Save & pair';

/** How long the "✓ Authorized" confirmation stays on the pair button before the
 *  popup flips to the import view, so the success state is actually seen. */
const AUTHORIZED_CONFIRM_MS = 800;

const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Pill labels carry a non-color glyph prefix so the connection state is
 * distinguishable without relying on color alone (deuteranopia-safe).
 */
const PILL_LABEL: Record<ConnectionStatus['phase'], string> = {
  searching: '○ Connecting…',
  not_paired: '⚠ Not paired',
  connected: '● Connected',
  app_not_running: '✕ App not running',
};

/** First status resolves within this budget, else fall back to the offline/Retry view. */
const STATUS_TIMEOUT_MS = 3_000;

/** Desktop deep link: launches/focuses the app on Settings → Browser extension
 *  with the pairing token highlighted. The click is the required user gesture;
 *  the browser may show its own "Open AI Job Hunter?" confirmation (expected). */
const PAIRING_DEEP_LINK = 'ajh://settings/extension';

/**
 * Last-known token state, cached so a transient `!ok` status reply (asleep or
 * just-woken service worker, message-channel race) can render the offline view
 * without spuriously telling a paired user to re-pair.
 */
let lastKnownHasToken = false;

/** Send a typed request to the background and return its typed response. */
async function send(req: PopupRequest): Promise<PopupResponse> {
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

function showView(phase: ConnectionStatus['phase']): void {
  els.views.import.hidden = phase !== 'connected';
  els.views.pair.hidden = phase !== 'not_paired';
  els.views.offline.hidden = phase !== 'app_not_running';
  els.views.searching.hidden = phase !== 'searching';
}

function render(status: ConnectionStatus): void {
  lastKnownHasToken = status.hasToken;
  els.pill.textContent = PILL_LABEL[status.phase];
  els.pill.className = `pill pill--${status.phase}`;
  showView(status.phase);
}

/**
 * Render the offline / Retry view without a fresh status from the background.
 * Used when the background is unreachable (transient `!ok`) or the first
 * status request times out, so the popup never stays stuck on the spinner.
 */
function renderOffline(): void {
  render({ phase: 'app_not_running', port: null, hasToken: lastKnownHasToken });
}

function setMsg(el: HTMLElement, text: string, tone: 'ok' | 'err' | 'muted'): void {
  el.textContent = text;
  el.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
}

async function refreshStatus(): Promise<void> {
  const res = await send({ kind: 'getStatus' });
  // resolveStatusResponse always returns a ConnectionStatus — offline fallback
  // when the background is unreachable; `!ok` path yields app_not_running.
  render(resolveStatusResponse(res, lastKnownHasToken));
}

/**
 * First status fetch with a timeout backstop: if the background does not answer
 * within {@link STATUS_TIMEOUT_MS}, fall back to the offline/Retry view rather
 * than spin indefinitely. A later status push or Retry will recover.
 */
async function refreshStatusWithTimeout(): Promise<void> {
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    renderOffline();
  }, STATUS_TIMEOUT_MS);
  try {
    const res = await send({ kind: 'getStatus' });
    if (timedOut) return;
    render(resolveStatusResponse(res, lastKnownHasToken));
  } finally {
    clearTimeout(timer);
  }
}

async function doImport(mode: ImportMode): Promise<void> {
  els.btnUrl.disabled = true;
  els.btnScan.disabled = true;
  setMsg(els.importMsg, mode === 'scan' ? 'Scanning page…' : 'Importing…', 'muted');
  try {
    const res = await send({ kind: 'import', mode, applied: els.chkApplied.checked });
    const { text, tone } = resolveImportResponse(res);
    setMsg(els.importMsg, text, tone);
  } finally {
    els.btnUrl.disabled = false;
    els.btnScan.disabled = false;
  }
}

async function savePairing(): Promise<void> {
  const value = els.tokenInput.value.trim();
  if (!looksLikeToken(value)) {
    setMsg(els.pairMsg, 'That does not look like a 64-character token.', 'err');
    return;
  }
  els.btnSaveToken.disabled = true;
  setMsg(els.pairMsg, 'Pairing…', 'muted');
  const res = await send({ kind: 'setToken', token: value });
  if (!res.ok) {
    setMsg(els.pairMsg, res.error, 'err');
    els.btnSaveToken.disabled = false;
    return;
  }
  // Confirm on the button itself, then flip to the import view after a beat so
  // the "Authorized" state is actually seen (refreshStatus hides the pair view).
  els.btnSaveToken.textContent = '✓ Authorized';
  setMsg(els.pairMsg, 'Paired.', 'ok');
  await delay(AUTHORIZED_CONFIRM_MS);
  await refreshStatus();
  if (!els.views.import.hidden) {
    // Connected view is now shown; move focus off the (hidden) token input.
    els.btnUrl.focus();
  } else {
    // Didn't reach the connected view (e.g. app went away) — restore the
    // actionable label so the pair button works again.
    els.btnSaveToken.textContent = PAIR_LABEL;
    els.btnSaveToken.disabled = false;
  }
}

async function unpair(): Promise<void> {
  await send({ kind: 'clearToken' });
  setMsg(els.importMsg, '', 'muted');
  // Restore the pair button to its actionable state for when the view returns.
  els.btnSaveToken.textContent = PAIR_LABEL;
  els.btnSaveToken.disabled = false;
  setMsg(els.pairMsg, '', 'muted');
  await refreshStatus();
  // Pairing view is now shown; move focus off the (hidden) import controls.
  if (!els.views.pair.hidden) els.tokenInput.focus();
}

/** Toggle the help popover open/closed and keep `aria-expanded` in sync. */
function toggleHelp(): void {
  const open = els.helpPopover.hidden;
  els.helpPopover.hidden = !open;
  els.btnHelp.setAttribute('aria-expanded', String(open));
}

async function retry(): Promise<void> {
  await send({ kind: 'reconnect' });
  await refreshStatus();
}

/** Open the desktop app at the extension-pairing settings via the custom URL
 *  scheme. `tabs.create` needs no permission; failures are swallowed so the
 *  popup never shows a disruptive error. */
async function openAppPairing(): Promise<void> {
  try {
    await browser.tabs.create({ url: PAIRING_DEEP_LINK });
  } catch {
    // No-op: the deep link is best-effort; the user can still pair manually.
  }
}

function wire(): void {
  els.btnUrl.addEventListener('click', () => void doImport('url'));
  els.btnScan.addEventListener('click', () => void doImport('scan'));
  els.btnSaveToken.addEventListener('click', () => void savePairing());
  els.tokenInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') void savePairing();
  });
  els.btnUnpair.addEventListener('click', () => void unpair());
  els.btnRetry.addEventListener('click', () => void retry());
  els.btnOpenApp.addEventListener('click', () => void openAppPairing());
  els.btnOpenSettings.addEventListener('click', () => void openAppPairing());
  els.btnHelp.addEventListener('click', toggleHelp);

  // Live status pushes from the background while the popup is open.
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
    if (res && res.ok && res.kind === 'status') render(res.status);
  });
}

wire();
void refreshStatusWithTimeout();
