/**
 * Popup controller (plain TS — deliberately NOT the app's React stack).
 *
 * It is a thin view over the background worker: it sends typed
 * {@link PopupRequest}s, delegates the connection status (pill/retry, pairing/
 * offline/outdated/searching views) to the shared `connection-status.ts`
 * module (ADR-046) — the SAME module the side panel mounts — and never talks
 * to the desktop bridge directly. Store reviewers test WITHOUT the desktop
 * app, so every state must render an explanation, never an error.
 */

import { browser } from '@wxt-dev/browser';

import { copyText, mountAnswerTools } from '../answer-tools/answer-tools';
import { mountConnectionStatus } from '../connection-status/connection-status';
import { IMPORT_LABEL_DEFAULT, IMPORT_LABEL_FOUND, mountJobTools } from '../job-tools/job-tools';
import { subscribeAnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { getAnswerToolsExpanded, setAnswerToolsExpanded } from '../lib/storage';

import './popup.css';

// `resolveStatusResponse` moved to `connection-status.ts` (ADR-046) — the pure
// helper the module's own tests cover.

// ── pure view-decision helpers (exported for unit tests) ─────────────────────

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
  views: {
    import: byId<HTMLElement>('view-import'),
  },
  connectionPillHost: byId<HTMLDivElement>('connection-pill-host'),
  connectionViewsHost: byId<HTMLDivElement>('connection-views-host'),
  btnMarkApplied: byId<HTMLButtonElement>('btn-mark-applied'),
  answerTools: byId<HTMLDetailsElement>('answer-tools'),
  answerToolsHost: byId<HTMLDivElement>('answer-tools-host'),
  jobToolsHost: byId<HTMLDivElement>('job-tools-host'),
  btnOpenPanel: byId<HTMLButtonElement>('btn-open-panel'),
  appliedStatus: byId<HTMLParagraphElement>('applied-status'),
  importMsg: byId<HTMLParagraphElement>('import-msg'),
  unpairGroup: byId<HTMLElement>('unpair-group'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  btnHelp: byId<HTMLButtonElement>('btn-help'),
  helpPopover: byId<HTMLParagraphElement>('help-popover'),
};

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

function setMsg(el: HTMLElement, text: string, tone: 'ok' | 'err' | 'muted'): void {
  el.textContent = text;
  el.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
}

/**
 * The connection-status pill/retry + the four non-connected views — the SAME
 * component the side panel mounts (ADR-046). `onStatus` fires on every render
 * (fresh or repeated) and owns what this file used to do directly in its own
 * `render()`'s `else` branch: gate `view-import`'s visibility, the "Unpair
 * this device" visibility (keyed on `hasToken` alone — the help popover is
 * global, not scoped to any one phase), and reset the connected-only content
 * left over from a previous page. `onConnected` fires once per TRANSITION
 * into `connected` for the fire-and-forget auto-checks. `onPaired` moves
 * focus off the (now-hidden) token input onto the Import button job-tools
 * owns, once a fresh pair reaches the connected view.
 */
const connectionStatus = mountConnectionStatus(els.connectionPillHost, els.connectionViewsHost, {
  send,
  onStatus: (status) => {
    els.unpairGroup.hidden = !status.hasToken;
    els.views.import.hidden = status.phase !== 'connected';
    if (status.phase !== 'connected') {
      els.appliedStatus.hidden = true;
      els.appliedStatus.textContent = '';
      els.btnMarkApplied.hidden = true;
      els.btnMarkApplied.disabled = false;
      jobTools.reset();
      // The Answer-tools rows are NOT cleared here: they live in the shared
      // per-tab state, not in this popup instance, and losing connection to
      // the desktop is not a reason to throw away drafts the user can still
      // copy (ADR-044 decision 3 keeps the rows even after a navigation).
      answerTools.render(null);
    }
  },
  onConnected: () => {
    void runAppliedAutoCheck();
    jobTools.checkPage();
    // Opening the popup IS the gesture that grants `activeTab`, so it is the
    // right (and only free) moment to scan the page into the shared state.
    void runAnswerScan();
  },
  onPaired: () => {
    els.jobToolsHost.querySelector<HTMLButtonElement>('#btn-import')?.focus();
  },
});

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

async function unpair(): Promise<void> {
  await send({ kind: 'clearToken' });
  setMsg(els.importMsg, '', 'muted');
  await connectionStatus.refresh();
  // Pairing view is now shown (if reached); move focus off the (hidden)
  // import controls.
  connectionStatus.focusPairInputIfShown();
}

/** Toggle the help popover open/closed and keep `aria-expanded` in sync. */
function toggleHelp(): void {
  const open = els.helpPopover.hidden;
  els.helpPopover.hidden = !open;
  els.btnHelp.setAttribute('aria-expanded', String(open));
}

function wire(): void {
  els.btnMarkApplied.addEventListener('click', () => void doMarkApplied());
  // NOT `void openAnswerPanel()` behind an await: opening the panel needs the
  // user gesture this click IS, and any await before the call spends it.
  els.btnOpenPanel.addEventListener('click', openAnswerPanel);
  els.btnUnpair.addEventListener('click', () => void unpair());
  els.btnHelp.addEventListener('click', toggleHelp);
  // Persist the Answer-tools expand/collapse preference across popup opens —
  // a UI boolean only, not PII/job data. Fires on BOTH a user click on the
  // <summary> and a programmatic `.open` set (e.g. the stream-reattach
  // auto-expand), per the `toggle` event's spec — that is fine here, the
  // stored preference is just "what state it was last left in".
  els.answerTools.addEventListener('toggle', () => {
    void setAnswerToolsExpanded(els.answerTools.open);
  });

  // The streamed draft itself is NOT rendered from this push: the background
  // mirrors every chunk into the shared per-tab state, and the Answer-tools
  // component below is subscribed to it, so the popup and the panel show the
  // same stream without either of them owning it. What IS worth doing here
  // is surfacing a TERMINAL interruption on the shared status line, which the
  // row itself cannot say as loudly. (The `status` push is handled inside
  // `connectionStatus` itself — see its own `start()`.)
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
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

// `connectionStatus.start()` first: it registers the FIRST `onMessage`
// listener (the live status push), which `popup.test.ts` relies on finding
// at `mock.calls[0]` — `wire()`'s own `answerAssistProgress` listener must
// register second.
connectionStatus.start();
wire();
void bootstrapAnswerTools();
