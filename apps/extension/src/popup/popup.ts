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
};

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
  try {
    const res = await send({ kind: 'setToken', token: value });
    if (!res.ok) {
      setMsg(els.pairMsg, res.error, 'err');
      return;
    }
    setMsg(els.pairMsg, 'Paired.', 'ok');
    await refreshStatus();
    // Connected view is now shown; move focus off the (hidden) token input.
    if (!els.views.import.hidden) els.btnUrl.focus();
  } finally {
    els.btnSaveToken.disabled = false;
  }
}

async function unpair(): Promise<void> {
  await send({ kind: 'clearToken' });
  setMsg(els.importMsg, '', 'muted');
  await refreshStatus();
  // Pairing view is now shown; move focus off the (hidden) import controls.
  if (!els.views.pair.hidden) els.tokenInput.focus();
}

async function retry(): Promise<void> {
  await send({ kind: 'reconnect' });
  await refreshStatus();
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

  // Live status pushes from the background while the popup is open.
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
    if (res && res.ok && res.kind === 'status') render(res.status);
  });
}

wire();
void refreshStatusWithTimeout();
