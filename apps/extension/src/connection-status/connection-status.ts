/**
 * The connection-status pill/retry button + the four non-connected views
 * (pair / offline / outdated / searching), mounted by BOTH the popup and the
 * side panel (ADR-046). Moved out of popup.ts essentially unchanged: same
 * wire calls (`send({kind:'getStatus'|'setToken'|'clearToken'|'reconnect'})`),
 * same phase→view mapping, same offline-sticky / bad_token-message / pairing
 * confirmation behavior — only the DOM target changed (two injected hosts,
 * not popup.html's specific element ids) and the live-push listener + first
 * status fetch now live here instead of in each caller's own `wire()`.
 *
 * The "connected" content (`view-import` in the popup, the job/answer tools
 * in the panel) is deliberately OUT of scope for this module — each caller
 * owns what it shows while connected. {@link ConnectionStatusDeps.onStatus}
 * is how a caller stays in sync with every render (gating its own connected
 * content, and — popup only — the "Unpair this device" visibility keyed on
 * `hasToken` alone); {@link ConnectionStatusDeps.onConnected} is the
 * once-per-transition hook for a caller's own fire-and-forget checks (the
 * popup's `appliedCheck`/`fieldsProbe`/`answerScan` auto-checks); {@link
 * ConnectionStatusDeps.onPaired} is the one-shot "pairing just succeeded and
 * we reached connected" hook the popup uses to move focus onto its own
 * (now-visible) content.
 */

import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus, PopupRequest, PopupResponse } from '../lib/messages';
import { looksLikeToken } from '../lib/storage';

/**
 * Given a `getStatus` response, return the {@link ConnectionStatus} to render,
 * or the offline fallback if the response signals the background is
 * unreachable. Pure: no DOM access, no side effects.
 */
export function resolveStatusResponse(
  res: PopupResponse,
  lastKnownHasToken: boolean
): ConnectionStatus {
  if (res.ok && res.kind === 'status') return res.status;
  return { phase: 'app_not_running', port: null, hasToken: lastKnownHasToken };
}

/** Actionable label for the pairing button; restored after a failed/cleared pair. */
const PAIR_LABEL = 'Save & pair';

/** How long the "✓ Authorized" confirmation stays on the pair button before the
 *  caller's connected content is shown, so the success state is actually seen. */
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

function setMsg(el: HTMLElement, text: string, tone: 'ok' | 'err' | 'muted'): void {
  el.textContent = text;
  el.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
}

export interface ConnectionStatusDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  /** Fired on every render (a fresh status OR a repeated push while already
   *  connected) with the status just rendered — for state a caller must
   *  always keep in sync: gating its own connected-only content
   *  (`status.phase !== 'connected'`), and (popup only) the "Unpair this
   *  device" visibility, keyed on `hasToken` alone. */
  onStatus?: (status: ConnectionStatus) => void;
  /** Fired once per TRANSITION into `connected` — never on a repeated push
   *  while already connected. Mirrors the popup's original
   *  `lastRenderedPhase` guard for its fire-and-forget auto-checks
   *  (`appliedCheck`/`fieldsProbe`/`answerScan`). */
  onConnected?: () => void;
  /** Fired once, after a successful pairing reaches the connected phase
   *  (never on a failed/incomplete pair) — lets a caller move focus off the
   *  now-hidden token input onto its own connected content. Optional: the
   *  panel has no equivalent focus target and omits this. */
  onPaired?: () => void;
}

export interface ConnectionStatusView {
  /** Kick the first status fetch (bounded by a timeout fallback) and start
   *  listening for live pushes from the background. Call once at mount. */
  start: () => void;
  /** Re-fetch and re-render immediately — used by a caller's own actions
   *  that change pairing state outside this module (the popup's "Unpair this
   *  device", which lives in its own help popover, not in this module). */
  refresh: () => Promise<void>;
  /** Move focus into the pairing token input, but only if the pair view is
   *  currently shown — a no-op otherwise. */
  focusPairInputIfShown: () => void;
}

/**
 * Mount the connection-status pill/retry into `pillHost` and the four
 * non-connected views into `viewsHost`. Both the popup and the side panel
 * call this against the SAME `deps.send`, so the two surfaces are two views
 * of one background, never two implementations.
 */
export function mountConnectionStatus(
  pillHost: HTMLElement,
  viewsHost: HTMLElement,
  deps: ConnectionStatusDeps
): ConnectionStatusView {
  // ── DOM: retry + pill ─────────────────────────────────────────────────
  const btnRetry = document.createElement('button');
  btnRetry.id = 'btn-retry';
  btnRetry.className = 'retry';
  btnRetry.type = 'button';
  btnRetry.setAttribute('aria-label', 'Retry');
  btnRetry.title = 'Retry';
  btnRetry.hidden = true;
  btnRetry.innerHTML =
    '<svg viewBox="0 0 24 24" width="11" height="11" aria-hidden="true" fill="none" ' +
    'stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">' +
    '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" /><path d="M21 3v5h-5" /></svg>';

  const pill = document.createElement('span');
  pill.id = 'status-pill';
  pill.className = 'pill pill--searching';
  pill.setAttribute('role', 'status');
  pill.textContent = PILL_LABEL.searching;

  pillHost.prepend(btnRetry, pill);

  // ── DOM: the four non-connected views ──────────────────────────────────
  const viewPair = document.createElement('section');
  viewPair.id = 'view-pair';
  viewPair.className = 'view';
  viewPair.hidden = true;

  const pairHint = document.createElement('p');
  pairHint.className = 'hint';
  pairHint.innerHTML =
    "Paste the pairing token from the desktop app's <strong>Settings → Browser extension</strong>.";

  const tokenInput = document.createElement('input');
  tokenInput.id = 'token-input';
  tokenInput.className = 'token';
  tokenInput.type = 'text';
  tokenInput.setAttribute('inputmode', 'text');
  tokenInput.setAttribute('autocomplete', 'off');
  tokenInput.setAttribute('spellcheck', 'false');
  tokenInput.placeholder = '64-character pairing token';
  tokenInput.setAttribute('aria-label', 'Pairing token');

  const pairMsg = document.createElement('p');
  pairMsg.id = 'pair-msg';
  pairMsg.className = 'msg';
  pairMsg.setAttribute('role', 'status');
  pairMsg.setAttribute('aria-live', 'polite');

  const btnSaveToken = document.createElement('button');
  btnSaveToken.id = 'btn-save-token';
  btnSaveToken.className = 'btn btn--primary';
  btnSaveToken.type = 'button';
  btnSaveToken.textContent = PAIR_LABEL;

  const btnOpenSettings = document.createElement('button');
  btnOpenSettings.id = 'btn-open-settings';
  btnOpenSettings.className = 'link';
  btnOpenSettings.type = 'button';
  btnOpenSettings.textContent = 'Find my token';

  viewPair.append(pairHint, tokenInput, pairMsg, btnSaveToken, btnOpenSettings);

  const viewOffline = document.createElement('section');
  viewOffline.id = 'view-offline';
  viewOffline.className = 'view';
  viewOffline.hidden = true;

  const offlineHint = document.createElement('p');
  offlineHint.className = 'hint';
  offlineHint.textContent = "Don't have the desktop app yet?";

  const btnGetApp = document.createElement('button');
  btnGetApp.id = 'btn-get-app';
  btnGetApp.className = 'btn btn--primary';
  btnGetApp.type = 'button';
  btnGetApp.textContent = 'Get the app';

  viewOffline.append(offlineHint, btnGetApp);

  const viewOutdated = document.createElement('section');
  viewOutdated.id = 'view-outdated';
  viewOutdated.className = 'view';
  viewOutdated.hidden = true;

  const outdatedHint = document.createElement('p');
  outdatedHint.className = 'hint';
  outdatedHint.textContent =
    'Your AI Job Hunter desktop app is out of date. Update it to reconnect the extension.';

  const btnUpdateApp = document.createElement('button');
  btnUpdateApp.id = 'btn-update-app';
  btnUpdateApp.className = 'btn btn--primary';
  btnUpdateApp.type = 'button';
  btnUpdateApp.textContent = 'Update the app';

  viewOutdated.append(outdatedHint, btnUpdateApp);

  // Searching state: the pill alone conveys it; no body needed.
  const viewSearching = document.createElement('section');
  viewSearching.id = 'view-searching';
  viewSearching.className = 'view';

  viewsHost.append(viewPair, viewOffline, viewOutdated, viewSearching);

  // ── state ───────────────────────────────────────────────────────────────

  /** Last-known token state, cached so a transient `!ok` status reply (asleep
   *  or just-woken service worker, message-channel race) can render the
   *  offline view without spuriously telling a paired user to re-pair. */
  let lastKnownHasToken = false;

  /** Set to `true` once the offline (`app_not_running`) view has been shown.
   *  While `true`, a transient `searching` status from a background
   *  reconnect attempt does NOT swap out the offline guidance — see
   *  `render`'s early-return branch. Reset to `false` on a real outcome. */
  let hasShownOffline = false;

  /** The phase from the previous `render()` call — used to fire {@link
   *  ConnectionStatusDeps.onConnected} exactly once per TRANSITION into
   *  `connected`. */
  let lastRenderedPhase: ConnectionStatus['phase'] | null = null;

  function showView(phase: ConnectionStatus['phase']): void {
    // Show the pairing view for both not_paired and bad_token — the user
    // must enter a corrected token in both cases.
    viewPair.hidden = phase !== 'not_paired' && phase !== 'bad_token';
    viewOffline.hidden = phase !== 'app_not_running';
    // Outdated desktop: a distinct "update the desktop app" view (NOT the
    // pairing view — the token is fine; the app is too old to speak the v2
    // handshake).
    viewOutdated.hidden = phase !== 'outdated';
    viewSearching.hidden = phase !== 'searching';
  }

  function render(status: ConnectionStatus): void {
    lastKnownHasToken = status.hasToken;
    const enteringConnected = status.phase === 'connected' && lastRenderedPhase !== 'connected';
    lastRenderedPhase = status.phase;

    deps.onStatus?.(status);
    if (enteringConnected) deps.onConnected?.();

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
      hasShownOffline = false;
    }

    // If a transient reconnect attempt (`searching`) arrives AFTER the
    // offline view was shown, keep the offline guidance visible. Only
    // update the pill label and keep the Retry button so the user can see
    // the retry is happening, but do NOT swap the view.
    if (status.phase === 'searching' && hasShownOffline) {
      pill.textContent = PILL_LABEL.searching;
      pill.className = 'pill pill--searching';
      btnRetry.hidden = false;
      return;
    }

    pill.textContent = PILL_LABEL[status.phase];
    pill.className = `pill pill--${status.phase}`;
    // Retry lives left of the pill and makes sense when the app is
    // unreachable OR outdated (re-probe after the user updates the desktop app).
    btnRetry.hidden = status.phase !== 'app_not_running' && status.phase !== 'outdated';
    showView(status.phase);
    if (status.phase === 'bad_token') {
      setMsg(
        pairMsg,
        "That token didn't match — copy the current token from the desktop app's Settings and try again.",
        'err'
      );
    } else if (status.phase === 'not_paired') {
      pairMsg.textContent = '';
    }
  }

  /** Render the offline / Retry view without a fresh status from the
   *  background — used when the background is unreachable or the first
   *  status request times out. */
  function renderOffline(): void {
    render({ phase: 'app_not_running', port: null, hasToken: lastKnownHasToken });
  }

  async function refresh(): Promise<void> {
    try {
      const res = await deps.send({ kind: 'getStatus' });
      render(resolveStatusResponse(res, lastKnownHasToken));
    } catch {
      // A rejected sendMessage (e.g. the MV3 service worker asleep/crashed —
      // "Could not establish connection") must still land on SOME rendered
      // view, not propagate as an unhandled rejection out of this
      // fire-and-forget call.
      renderOffline();
    }
  }

  /** First status fetch with a timeout backstop: if the background does not
   *  answer within {@link STATUS_TIMEOUT_MS}, fall back to the offline/Retry
   *  view rather than spin indefinitely. A later status push or Retry will
   *  recover. A rejected `send()` (not just a slow one) hits the SAME
   *  fallback via the `catch` below — see `refresh`'s doc for why. */
  async function refreshWithTimeout(): Promise<void> {
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      renderOffline();
    }, STATUS_TIMEOUT_MS);
    try {
      const res = await deps.send({ kind: 'getStatus' });
      if (timedOut) return;
      render(resolveStatusResponse(res, lastKnownHasToken));
    } catch {
      if (!timedOut) renderOffline();
    } finally {
      clearTimeout(timer);
    }
  }

  /** Poll `getStatus` until the phase leaves `searching` or the attempts run
   *  out — a safety net for a missed live push (MV3 race / just-woken
   *  worker) after a successful pair. */
  async function refreshUntilSettled(attempts = 5, gapMs = 600): Promise<void> {
    for (let i = 0; i < attempts; i += 1) {
      try {
        const res = await deps.send({ kind: 'getStatus' });
        const status = resolveStatusResponse(res, lastKnownHasToken);
        render(status);
        if (status.phase !== 'searching') return;
      } catch {
        renderOffline();
        return;
      }
      if (i < attempts - 1) await delay(gapMs);
    }
  }

  async function savePairing(): Promise<void> {
    const value = tokenInput.value.trim();
    if (!looksLikeToken(value)) {
      setMsg(pairMsg, 'That does not look like a 64-character token.', 'err');
      return;
    }
    btnSaveToken.disabled = true;
    setMsg(pairMsg, 'Pairing…', 'muted');
    try {
      const res = await deps.send({ kind: 'setToken', token: value });
      if (!res.ok) {
        setMsg(pairMsg, res.error, 'err');
        btnSaveToken.textContent = PAIR_LABEL;
        btnSaveToken.disabled = false;
        return;
      }
      btnSaveToken.textContent = '✓ Authorized';
      setMsg(pairMsg, 'Paired.', 'ok');
      await delay(AUTHORIZED_CONFIRM_MS);
      await refreshUntilSettled();
      if (lastRenderedPhase === 'connected') {
        deps.onPaired?.();
      } else {
        // Didn't reach the connected view (e.g. app went away) — restore the
        // actionable label so the pair button works again.
        btnSaveToken.textContent = PAIR_LABEL;
        btnSaveToken.disabled = false;
      }
    } catch {
      setMsg(pairMsg, 'Pairing failed. Please retry.', 'err');
      btnSaveToken.textContent = PAIR_LABEL;
      btnSaveToken.disabled = false;
    }
  }

  async function retry(): Promise<void> {
    await deps.send({ kind: 'reconnect' });
    await refresh();
  }

  async function openAppPairing(): Promise<void> {
    try {
      await browser.tabs.create({ url: PAIRING_DEEP_LINK });
    } catch {
      // No-op: the deep link is best-effort; the user can still pair manually.
    }
  }

  async function getApp(): Promise<void> {
    try {
      await browser.tabs.create({ url: GET_APP_URL });
    } catch {
      // No-op: best-effort.
    }
  }

  btnSaveToken.addEventListener('click', () => void savePairing());
  tokenInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') void savePairing();
  });
  btnRetry.addEventListener('click', () => void retry());
  btnOpenSettings.addEventListener('click', () => void openAppPairing());
  btnGetApp.addEventListener('click', () => void getApp());
  // The outdated-desktop view sends the user to the same download page
  // (which serves the latest build) to update their app.
  btnUpdateApp.addEventListener('click', () => void getApp());

  function start(): void {
    // Live status pushes from the background — a genuine push (broadcastStatus
    // on every bridge phase change), not something either surface has to poll
    // for, so this listener is the ONLY thing keeping a long-lived panel
    // in sync after its first fetch below.
    browser.runtime.onMessage.addListener((message: unknown) => {
      const res = message as PopupResponse;
      if (res && res.ok && res.kind === 'status') render(res.status);
    });
    void refreshWithTimeout();
  }

  function focusPairInputIfShown(): void {
    if (!viewPair.hidden) tokenInput.focus();
  }

  return { start, refresh, focusPairInputIfShown };
}
