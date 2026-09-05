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

import { copyText, mountAnswerTools } from '../answer-tools/answer-tools';
import { IMPORT_LABEL_DEFAULT, IMPORT_LABEL_FOUND, mountJobTools } from '../job-tools/job-tools';
import { subscribeAnswerState } from '../lib/answer-state';
import type { ConnectionStatus, PopupRequest, PopupResponse } from '../lib/messages';
import { getAnswerToolsExpanded, looksLikeToken, setAnswerToolsExpanded } from '../lib/storage';

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

/** Format an epoch-ms timestamp as a short local date (e.g. "Jun 12", or
 *  "Jun 12, 2025" when the date's year differs from the current year) —
 *  popup-local formatting, no date library. */
function formatShortDate(epochMs: number): string {
  const date = new Date(epochMs);
  const opts: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric' };
  if (date.getFullYear() !== new Date().getFullYear()) opts.year = 'numeric';
  return date.toLocaleDateString(undefined, opts);
}

/**
 * Given an `appliedCheck` response, return the status line to render above the
 * import controls, or `null` when nothing should be shown — not found, or ANY
 * error (the check is a silent best-effort enhancement, never a blocker; see
 * `runAppliedCheck` in background.ts, which already folds every failure mode
 * into `result.found === false`).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAppliedStatusLine(res: PopupResponse): string | null {
  if (!res.ok || res.kind !== 'appliedCheck') return null;
  const { result } = res;
  if (result.error || !result.found) return null;

  const title = result.title?.trim();
  const lead = title ? `“${title}”` : null;
  if (!result.status || result.status === 'saved') {
    return lead ? `${lead} is saved in your pipeline.` : 'Saved in your pipeline.';
  }
  const when = typeof result.appliedAt === 'number' ? formatShortDate(result.appliedAt) : null;
  if (lead && when) return `${lead} is already in your pipeline — applied ${when}.`;
  if (lead) return `${lead} is already in your pipeline.`;
  if (when) return `Already in your pipeline — applied ${when}.`;
  return 'Already in your pipeline.';
}

/**
 * The import button's label: unchanged when no existing Application was found
 * for the active tab's url, {@link IMPORT_LABEL_FOUND} when one was. Any
 * non-found/error outcome (including one still in flight) keeps the default.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveImportButtonLabel(res: PopupResponse): string {
  if (res.ok && res.kind === 'appliedCheck' && !res.result.error && res.result.found) {
    return IMPORT_LABEL_FOUND;
  }
  return IMPORT_LABEL_DEFAULT;
}

/**
 * Whether the "Mark as applied" button should show: only for a found
 * Application whose status is EXPLICITLY `saved` — the ONLY status this
 * write's CAS precondition can ever transition FROM (the bridge's
 * `saved → applied` compare-and-set requires the current status to already
 * be `saved`; an absent/unknown status is not the same guarantee). Any other
 * status (already applied, mid-pipeline, missing, or not found/error) keeps
 * the button hidden; those cases use the existing "I already applied" import
 * checkbox, not this button.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveShowMarkAppliedButton(res: PopupResponse): boolean {
  if (!res.ok || res.kind !== 'appliedCheck') return false;
  const { result } = res;
  if (result.error || !result.found) return false;
  return result.status === 'saved';
}

/**
 * Given a `statusUpdate` response, return the message text + tone. UNLIKE
 * `resolveAppliedStatusLine`/`resolveImportButtonLabel` (which fold every
 * failure into "render nothing" — this is a passive, best-effort check),
 * this verb's errors ARE shown: it answers a deliberate click. A
 * transport-level `ok:false` surfaces its `error`; a resolved
 * `result.ok === false` (the desktop's own refusal — no match / wrong
 * starting status) surfaces `result.error`.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveMarkAppliedResponse(res: PopupResponse): {
  text: string;
  tone: 'ok' | 'err';
} {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'statusUpdate') {
    return { text: 'Unexpected response — please retry.', tone: 'err' };
  }
  const { result } = res;
  if (!result.ok) {
    return { text: result.error ?? 'Could not mark this job as applied.', tone: 'err' };
  }
  return { text: 'Marked as applied.', tone: 'ok' };
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
    outdated: byId<HTMLElement>('view-outdated'),
    searching: byId<HTMLElement>('view-searching'),
  },
  btnMarkApplied: byId<HTMLButtonElement>('btn-mark-applied'),
  answerTools: byId<HTMLDetailsElement>('answer-tools'),
  answerToolsHost: byId<HTMLDivElement>('answer-tools-host'),
  jobToolsHost: byId<HTMLDivElement>('job-tools-host'),
  btnOpenPanel: byId<HTMLButtonElement>('btn-open-panel'),
  appliedStatus: byId<HTMLParagraphElement>('applied-status'),
  importMsg: byId<HTMLParagraphElement>('import-msg'),
  unpairGroup: byId<HTMLElement>('unpair-group'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  tokenInput: byId<HTMLInputElement>('token-input'),
  pairMsg: byId<HTMLParagraphElement>('pair-msg'),
  btnSaveToken: byId<HTMLButtonElement>('btn-save-token'),
  btnRetry: byId<HTMLButtonElement>('btn-retry'),
  btnOpenSettings: byId<HTMLButtonElement>('btn-open-settings'),
  btnHelp: byId<HTMLButtonElement>('btn-help'),
  helpPopover: byId<HTMLParagraphElement>('help-popover'),
  btnGetApp: byId<HTMLButtonElement>('btn-get-app'),
  btnUpdateApp: byId<HTMLButtonElement>('btn-update-app'),
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
  outdated: '⟳ Update the app',
  bad_token: '✕ Wrong token',
};

/** First status resolves within this budget, else fall back to the offline/Retry view. */
const STATUS_TIMEOUT_MS = 3_000;

/** Desktop deep link: launches/focuses the app on Settings → Browser extension
 *  with the pairing token highlighted. The click is the required user gesture;
 *  the browser may show its own "Open AI Job Hunter?" confirmation (expected). */
const PAIRING_DEEP_LINK = 'ajh://settings/extension';

/** Public download page, offered in the offline view for users who don't yet
 *  have the desktop app installed. */
const GET_APP_URL = 'https://aijobhunter.app/download';

/**
 * Last-known token state, cached so a transient `!ok` status reply (asleep or
 * just-woken service worker, message-channel race) can render the offline view
 * without spuriously telling a paired user to re-pair.
 */
let lastKnownHasToken = false;

/**
 * Set to `true` once the offline (`app_not_running`) view has been shown.
 * While `true`, a transient `searching` status from a background reconnect
 * attempt does NOT swap out the offline guidance — the user already knows the
 * app is unreachable; briefly hiding the "Get the app" content on every retry
 * cycle is disorienting. Reset to `false` when a real outcome arrives
 * (`connected`, `not_paired`, or `bad_token`).
 */
let hasShownOffline = false;

/**
 * The phase from the previous `render()` call. Used to fire the
 * fire-and-forget `appliedCheck` auto-check exactly once per TRANSITION into
 * `connected` — not on every status push while already connected (a repeated
 * live-status push during a stable connection must not re-fire it), but a
 * genuine reconnect after a drop naturally re-checks, since that is a fresh
 * transition too.
 */
let lastRenderedPhase: ConnectionStatus['phase'] | null = null;

/**
 * The tab this popup is looking at. Read once at bootstrap via `tabs.query`,
 * which returns a tab's ID without the `tabs` permission (only its url/title
 * are gated behind that), so the shared state stays keyed per tab while
 * `tabs` stays on the manifest denylist.
 */
let activeTabId: number | null = null;

/** Send a typed request to the background and return its typed response. */
async function send(req: PopupRequest): Promise<PopupResponse> {
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

/**
 * The Answer-tools section — the SAME component the side panel mounts, over
 * the SAME shared state (ADR-044 decision 1). Everything the two surfaces
 * have to agree about lives in that state; this is only a view of it.
 */
const answerTools = mountAnswerTools(els.answerToolsHost, { send, copy: copyText });

/**
 * The Import/Check-fit/Fill/Save-answers controls — the SAME component the
 * side panel mounts (see `job-tools.ts`'s doc). `onAnswerToolsVisibility`
 * forwards the fields probe's other signal to the ONE thing this module has
 * no opinion on: the `<details>` disclosure around the Answer-tools section
 * above, which only the popup renders.
 */
const jobTools = mountJobTools(els.jobToolsHost, {
  send,
  onAnswerToolsVisibility: (visible) => {
    els.answerTools.hidden = !visible;
  },
});

/**
 * Open the answer panel. Called SYNCHRONOUSLY from the click handler on both
 * browsers, because both `chrome.sidePanel.open` and
 * `browser.sidebarAction.open` require a user gesture and an await before
 * either one spends it. There is no `setOptions` call to make first: the
 * panel's path is declared in the manifest, which is exactly why doing it
 * that way is safe here. The toolbar click cannot open the panel itself — a
 * declared `default_popup` takes priority over that behaviour, which is why
 * this control exists at all (ADR-044 decision 2).
 */
function openAnswerPanel(): void {
  const chromePanel = (browser as { sidePanel?: { open(o: { tabId: number }): Promise<void> } })
    .sidePanel;
  if (chromePanel && activeTabId !== null) {
    void chromePanel.open({ tabId: activeTabId }).catch(() => {
      setMsg(els.importMsg, 'Could not open the side panel.', 'err');
    });
    return;
  }
  if (chromePanel) {
    // The panel API exists (Chrome) but bootstrap hasn't resolved a tab id
    // yet — report THAT, not "this browser has no side panel", which is
    // false here and points the user at the wrong fix.
    setMsg(els.importMsg, 'Could not open the side panel for this tab.', 'err');
    return;
  }
  const sidebar = (browser as { sidebarAction?: { open(): Promise<void> } }).sidebarAction;
  if (!sidebar) {
    setMsg(els.importMsg, 'This browser has no side panel — the tools above still work.', 'err');
    return;
  }
  void sidebar.open().catch(() => {
    setMsg(els.importMsg, 'Could not open the sidebar.', 'err');
  });
}

/**
 * Rescan the page into the shared answer state. Fire-and-forget: it runs off
 * a gesture the user made for another reason (opening the popup, saving their
 * answers), so a failure must never talk over what they actually asked for —
 * the Answer-tools section shows its own empty state instead.
 */
function runAnswerScan(): void {
  void send({ kind: 'answerScan' }).catch(() => undefined);
}

function showView(phase: ConnectionStatus['phase']): void {
  els.views.import.hidden = phase !== 'connected';
  // Show the pairing view for both not_paired and bad_token — the user must
  // enter a corrected token in both cases.
  els.views.pair.hidden = phase !== 'not_paired' && phase !== 'bad_token';
  els.views.offline.hidden = phase !== 'app_not_running';
  // Outdated desktop: a distinct "update the desktop app" view (NOT the pairing
  // view — the token is fine; the app is too old to speak the v2 handshake).
  els.views.outdated.hidden = phase !== 'outdated';
  els.views.searching.hidden = phase !== 'searching';
}

function render(status: ConnectionStatus): void {
  lastKnownHasToken = status.hasToken;
  // The help popover is global (not scoped to any one view/phase) — only show
  // "Unpair this device" while there is actually something to unpair.
  els.unpairGroup.hidden = !status.hasToken;

  if (status.phase === 'connected') {
    // Fire-and-forget, on each transition INTO connected (never on a repeated
    // push while already connected) — never awaited here, so it can never delay
    // this render.
    if (lastRenderedPhase !== 'connected') {
      void runAppliedAutoCheck();
      jobTools.checkPage();
      // Opening the popup IS the gesture that grants `activeTab`, so it is the
      // right (and only free) moment to scan the page into the shared state.
      void runAnswerScan();
    }
  } else {
    // Left (or never entered) `connected` — clear any status line/button label
    // left over from a previous page so it can't flash stale for the next one
    // before its own check resolves.
    els.appliedStatus.hidden = true;
    els.appliedStatus.textContent = '';
    els.btnMarkApplied.hidden = true;
    els.btnMarkApplied.disabled = false;
    // Resets the Import label, the match-fit card, and the Form group's
    // visibility — everything job-tools now owns that this branch used to
    // reset directly (a stale "no fields on the previous page" hide, or a
    // stale in-flight fieldsProbe response, must never linger onto a fresh
    // page before its own probe resolves).
    jobTools.reset();
    // The Answer-tools rows are NOT cleared here: they live in the shared
    // per-tab state, not in this popup instance, and losing connection to the
    // desktop is not a reason to throw away drafts the user can still copy
    // (ADR-044 decision 3 keeps the rows even after a navigation). Rendering
    // `null` only empties what THIS view is showing.
    answerTools.render(null);
  }
  lastRenderedPhase = status.phase;

  // Track whether the offline view has been shown so we can suppress the
  // flickering "Connecting…" spinner during background reconnect attempts.
  if (status.phase === 'app_not_running') {
    hasShownOffline = true;
  } else if (
    status.phase === 'connected' ||
    status.phase === 'not_paired' ||
    status.phase === 'bad_token' ||
    status.phase === 'outdated'
  ) {
    // A real outcome arrived — reset so the next session starts fresh.
    hasShownOffline = false;
  }

  // If a transient reconnect attempt (`searching`) arrives AFTER the offline
  // view was shown, keep the offline guidance visible. Only update the pill
  // label and keep the Retry button so the user can see the retry is happening,
  // but do NOT swap the view — that would hide the "Get the app" content.
  if (status.phase === 'searching' && hasShownOffline) {
    els.pill.textContent = PILL_LABEL.searching;
    els.pill.className = `pill pill--searching`;
    els.btnRetry.hidden = false;
    return;
  }

  els.pill.textContent = PILL_LABEL[status.phase];
  els.pill.className = `pill pill--${status.phase}`;
  // Retry lives in the header (left of the pill) and makes sense when the app is
  // unreachable OR outdated (re-probe after the user updates the desktop app).
  els.btnRetry.hidden = status.phase !== 'app_not_running' && status.phase !== 'outdated';
  showView(status.phase);
  // On bad_token, surface a clear error in the pairing view so the user knows
  // they need to copy the current token from the desktop app's Settings.
  if (status.phase === 'bad_token') {
    setMsg(
      els.pairMsg,
      "That token didn't match — copy the current token from the desktop app's Settings and try again.",
      'err'
    );
  } else if (status.phase === 'not_paired') {
    els.pairMsg.textContent = '';
  }
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

/**
 * Poll `getStatus` until the phase leaves `searching` (the bridge connection has
 * settled to connected / not_paired / app_not_running) or the attempts run out.
 * Renders each result. A safety net so the popup never strands on the "searching"
 * spinner when the background's live status push is missed (MV3 race / just-woken
 * worker). The live `onMessage` push still updates the view independently.
 */
async function refreshUntilSettled(attempts = 5, gapMs = 600): Promise<void> {
  for (let i = 0; i < attempts; i += 1) {
    try {
      const res = await send({ kind: 'getStatus' });
      const status = resolveStatusResponse(res, lastKnownHasToken);
      render(status);
      if (status.phase !== 'searching') return;
    } catch {
      // A transient MV3 message-channel rejection here must NOT bubble into the
      // savePairing catch (a false "Pairing failed" after a successful pair). The
      // live status push / next popup open recovers; show offline and stop.
      renderOffline();
      return;
    }
    if (i < attempts - 1) await delay(gapMs);
  }
}

/**
 * Generation counter guarding {@link runAppliedAutoCheck} against a stale
 * in-flight response. A disconnect→reconnect re-enters `connected` and fires
 * a fresh check while the previous one may still be awaiting `send()`; if the
 * stale one resolves (or rejects) AFTER the newer check has started, it must
 * not overwrite the newer result.
 */
let appliedCheckGeneration = 0;

/**
 * Run the fire-and-forget `appliedCheck` and render its outcome: the status
 * line above the import controls, plus the adaptive import-button label.
 * `runAppliedCheck` in background.ts already folds every failure mode into
 * `ok:true, result:{found:false}`, so the try/catch here only guards a
 * transport-level rejection (message-channel closed) — either way nothing is
 * ever shown but "no line, default label".
 */
async function runAppliedAutoCheck(): Promise<void> {
  appliedCheckGeneration += 1;
  const myGeneration = appliedCheckGeneration;
  // Clear synchronously before the request goes out (belt-and-suspenders): if
  // render() re-enters `connected` for a new page while a previous check is
  // still in flight, the previous page's line/label must not linger while
  // this fresh one resolves.
  els.appliedStatus.hidden = true;
  els.appliedStatus.textContent = '';
  jobTools.setImportLabel(IMPORT_LABEL_DEFAULT);
  els.btnMarkApplied.hidden = true;
  els.btnMarkApplied.disabled = false;
  try {
    const res = await send({ kind: 'appliedCheck' });
    // A newer check started while this one was in flight — its result (or the
    // DOM state the newer check already wrote) must win; bail before touching
    // the DOM.
    if (myGeneration !== appliedCheckGeneration) return;
    const line = resolveAppliedStatusLine(res);
    els.appliedStatus.hidden = line === null;
    els.appliedStatus.textContent = line ?? '';
    jobTools.setImportLabel(resolveImportButtonLabel(res));
    // Only a found+saved result shows the button — reset disabled here too,
    // so a re-fire after a successful "Mark as applied" click (which left the
    // button disabled) ends re-enabled for whatever this fresh check renders.
    els.btnMarkApplied.hidden = !resolveShowMarkAppliedButton(res);
    els.btnMarkApplied.disabled = false;
  } catch {
    if (myGeneration !== appliedCheckGeneration) return;
    els.appliedStatus.hidden = true;
    els.appliedStatus.textContent = '';
    jobTools.setImportLabel(IMPORT_LABEL_DEFAULT);
    els.btnMarkApplied.hidden = true;
    els.btnMarkApplied.disabled = false;
  }
}

/**
 * Click handler for "Mark as applied". Sends `status.update` and shows the
 * result in the existing message area — UNLIKE the passive auto-check,
 * failures ARE shown here (this is a deliberate click action). On success it
 * re-fires {@link runAppliedAutoCheck} (the SAME generation-guarded path
 * every other applied.check render goes through) instead of hand-rolling a
 * DOM update, so the status line flips to the applied wording and this
 * button hides itself once the fresh check confirms it.
 */
async function doMarkApplied(): Promise<void> {
  els.btnMarkApplied.disabled = true;
  setMsg(els.importMsg, 'Marking as applied…', 'muted');
  try {
    const res = await send({ kind: 'statusUpdate' });
    const { text, tone } = resolveMarkAppliedResponse(res);
    setMsg(els.importMsg, text, tone);
    if (tone === 'ok') {
      void runAppliedAutoCheck();
    } else {
      els.btnMarkApplied.disabled = false;
    }
  } catch {
    // A transport/messaging rejection must not strand the button disabled.
    setMsg(els.importMsg, 'Could not mark this job as applied. Please retry.', 'err');
    els.btnMarkApplied.disabled = false;
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
      els.btnSaveToken.textContent = PAIR_LABEL;
      els.btnSaveToken.disabled = false;
      return;
    }
    // Confirm on the button itself, then flip to the import view after a beat so
    // the "Authorized" state is actually seen (refreshStatus hides the pair view).
    els.btnSaveToken.textContent = '✓ Authorized';
    setMsg(els.pairMsg, 'Paired.', 'ok');
    await delay(AUTHORIZED_CONFIRM_MS);
    await refreshUntilSettled();
    if (!els.views.import.hidden) {
      // Connected view is now shown; move focus off the (hidden) token
      // input, onto the Import button job-tools now owns.
      els.jobToolsHost.querySelector<HTMLButtonElement>('#btn-import')?.focus();
    } else {
      // Didn't reach the connected view (e.g. app went away) — restore the
      // actionable label so the pair button works again.
      els.btnSaveToken.textContent = PAIR_LABEL;
      els.btnSaveToken.disabled = false;
    }
  } catch {
    // A transport/refresh rejection must never strand the button disabled and
    // labelled "Authorized" — always restore the actionable state.
    setMsg(els.pairMsg, 'Pairing failed. Please retry.', 'err');
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

/** Open the public download page in a new tab so a user without the desktop
 *  app can install it. Best-effort; failures are swallowed. */
async function getApp(): Promise<void> {
  try {
    await browser.tabs.create({ url: GET_APP_URL });
  } catch {
    // No-op: best-effort.
  }
}

function wire(): void {
  els.btnMarkApplied.addEventListener('click', () => void doMarkApplied());
  // NOT `void openAnswerPanel()` behind an await: opening the panel needs the
  // user gesture this click IS, and any await before the call spends it.
  els.btnOpenPanel.addEventListener('click', openAnswerPanel);
  els.btnSaveToken.addEventListener('click', () => void savePairing());
  els.tokenInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') void savePairing();
  });
  els.btnUnpair.addEventListener('click', () => void unpair());
  els.btnRetry.addEventListener('click', () => void retry());
  els.btnOpenSettings.addEventListener('click', () => void openAppPairing());
  els.btnGetApp.addEventListener('click', () => void getApp());
  // The outdated-desktop view sends the user to the same download page (which
  // serves the latest build) to update their app.
  els.btnUpdateApp.addEventListener('click', () => void getApp());
  els.btnHelp.addEventListener('click', toggleHelp);
  // Persist the Answer-tools expand/collapse preference across popup opens —
  // a UI boolean only, not PII/job data. Fires on BOTH a user click on the
  // <summary> and a programmatic `.open` set (e.g. the stream-reattach
  // auto-expand), per the `toggle` event's spec — that is fine here, the
  // stored preference is just "what state it was last left in".
  els.answerTools.addEventListener('toggle', () => {
    void setAnswerToolsExpanded(els.answerTools.open);
  });

  // Live status pushes from the background while the popup is open.
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
    if (res && res.ok && res.kind === 'status') render(res.status);
    // The streamed draft itself is NOT rendered from this push any more: the
    // background mirrors every chunk into the shared per-tab state, and the
    // Answer-tools component below is subscribed to it, so the popup and the
    // panel show the same stream without either of them owning it. What is
    // still worth doing here is surfacing a TERMINAL interruption on the
    // shared status line, which the row itself cannot say as loudly.
    if (res && res.ok && res.kind === 'answerAssistProgress' && res.done && res.interrupted) {
      setMsg(els.importMsg, 'Connection interrupted — here is what arrived so far.', 'err');
    }
  });
}

/**
 * Apply the persisted Answer-tools expand/collapse preference, then subscribe
 * the shared Answer-tools component to THIS tab's state — in that order, so a
 * buffered draft (which always wins) is never immediately re-collapsed by a
 * stale "collapsed" preference applied after it.
 *
 * The subscription is what replaces the old popup-open reattach: the
 * background mirrors an in-flight stream into the state, so a popup that
 * opens mid-stream renders it from the first `render` call rather than
 * querying for it. A tab id that cannot be read (no active tab) leaves the
 * component on its empty state, which is also what it shows before the first
 * scan.
 *
 * Exported (unlike the other `do*`/render helpers) because nothing wires a
 * user click to re-run this bootstrap — it only ever runs once, automatically,
 * at popup load — so it has no other seam for tests to drive it directly.
 */
export async function bootstrapAnswerTools(): Promise<void> {
  try {
    els.answerTools.open = await getAnswerToolsExpanded();
  } catch {
    // Best-effort — a storage read hiccup just keeps the collapsed default.
  }
  try {
    const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
    activeTabId = typeof tab?.id === 'number' ? tab.id : null;
    // The "N questions · M to go" summary is rendered ONCE, by the panel body
    // itself (`answer-tools.ts`'s `render()`) — it needs it there for the side
    // panel, which has no `<summary>` disclosure. Duplicating it into this
    // `<summary>` line as well showed it twice in the popup.
    if (activeTabId !== null)
      subscribeAnswerState(activeTabId, (state) => answerTools.render(state));
  } catch {
    // Best-effort — no tab id just means the section renders its empty state.
  }
}

wire();
void refreshStatusWithTimeout();
void bootstrapAnswerTools();
