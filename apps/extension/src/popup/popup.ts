/**
 * Popup controller (plain TS — deliberately NOT the app's React stack).
 *
 * The popup is the LAUNCHER surface of the PR0 redesign (`.claude/scratch/
 * extension-round-design.md`): a page-context card, exactly three gesture
 * actions (Import / Check fit / Fill — `job-tools.ts`'s `hideSaveAnswers`),
 * a quiet "Open the panel →", and a best-effort notice line. The interactive
 * Answer-tools UI moved fully into the side panel's Answers tab (ADR-044
 * decision 1 reversed by the owner) — this file only reports a count.
 *
 * It is a thin view over the background worker: it sends typed
 * {@link PopupRequest}s, delegates the connection status (pill/retry, pairing/
 * offline/outdated/searching views) to the shared `connection-status.ts`
 * module (ADR-046) — the SAME module the side panel mounts — and never talks
 * to the desktop bridge directly. Store reviewers test WITHOUT the desktop
 * app, so every state must render an explanation, never an error.
 */

import { browser } from '@wxt-dev/browser';

import { mountConnectionStatus } from '../connection-status/connection-status';
import { resolveJobStatusView } from '../job-status/job-status';
import { IMPORT_LABEL_DEFAULT, IMPORT_LABEL_FOUND, mountJobTools } from '../job-tools/job-tools';
import { type AnswerState, subscribeAnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { bootTheme } from '../lib/theme';

import './popup.css';

// Apply the Settings → Appearance → Theme choice before anything else renders
// (best-effort — see `lib/theme.ts`'s doc for the system-default fallback).
void bootTheme();

// ── pure view-decision helpers (exported for unit tests) ─────────────────────

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
 * `resolveImportButtonLabel` (which folds every failure into "render
 * nothing" — the auto-check is a passive, best-effort enhancement), this
 * verb's errors ARE shown: it answers a deliberate click. A transport-level
 * `ok:false` surfaces its `error`; a resolved `result.ok === false` (the
 * desktop's own refusal — no match / wrong starting status) surfaces
 * `result.error`.
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

/**
 * The popup's best-effort "notice line" (PR0 §2) — a passive count of
 * answers already ready for this page, in place of the panel's full
 * Answer-tools UI. `company` is not part of {@link AnswerState} (it only
 * carries `origin`, the gesture-captured url origin), so this deliberately
 * does not name the employer the way the design record's example line does —
 * same honest-narrowing discipline as `job-status.ts`'s own doc for the same
 * missing field.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAnswersNoticeLine(state: AnswerState | null): string | null {
  if (!state) return null;
  const ready = state.rows.filter((row) => row.status !== 'empty').length;
  if (ready === 0) return null;
  return `${ready} answer${ready === 1 ? '' : 's'} ready on this page.`;
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
  jobCard: byId<HTMLDivElement>('job-card'),
  jobCardTitle: byId<HTMLParagraphElement>('job-card-title'),
  appliedStatus: byId<HTMLSpanElement>('applied-status'),
  btnMarkApplied: byId<HTMLButtonElement>('btn-mark-applied'),
  jobToolsHost: byId<HTMLDivElement>('job-tools-host'),
  btnOpenPanel: byId<HTMLButtonElement>('btn-open-panel'),
  answersNotice: byId<HTMLParagraphElement>('answers-notice'),
  importMsg: byId<HTMLParagraphElement>('import-msg'),
  unpairGroup: byId<HTMLElement>('unpair-group'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  btnHelp: byId<HTMLButtonElement>('btn-help'),
  menu: byId<HTMLDivElement>('menu'),
  menuHelp: byId<HTMLButtonElement>('menu-help'),
  menuSettings: byId<HTMLButtonElement>('menu-settings'),
  menuAbout: byId<HTMLButtonElement>('menu-about'),
  helpPopover: byId<HTMLParagraphElement>('help-popover'),
  aboutPopover: byId<HTMLDivElement>('about-popover'),
  aboutVersion: byId<HTMLParagraphElement>('about-version'),
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
 * The Import/Check-fit/Fill controls — the SAME component the side panel
 * mounts (see `job-tools.ts`'s doc), with `hideSaveAnswers` so the popup
 * shows exactly the three gesture actions PR0 §2 asks for; "Save my answers"
 * lives only in the panel's Answers tab now.
 */
const jobTools = mountJobTools(els.jobToolsHost, { send, hideSaveAnswers: true });

/**
 * Open the side panel. Called SYNCHRONOUSLY from the click handler on both
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
 * a gesture the user made for another reason (opening the popup), so a
 * failure must never talk over what they actually asked for. The scan feeds
 * the panel's Answers tab AND this popup's own notice line.
 */
function runAnswerScan(): void {
  void send({ kind: 'answerScan' }).catch(() => undefined);
}

function setMsg(el: HTMLElement, text: string, tone: 'ok' | 'err' | 'muted'): void {
  el.textContent = text;
  el.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
  // An empty status line still reserves `.msg`'s min-height — costly in the
  // popup's tight 360×520 no-scroll budget (PR0 §2) when #import-msg (the
  // only caller) has nothing to say, which is most of the time.
  el.hidden = text.length === 0;
}

/**
 * The connection-status pill/retry + the four non-connected views — the SAME
 * component the side panel mounts (ADR-046). `onStatus` fires on every render
 * (fresh or repeated) and owns what this file used to do directly in its own
 * `render()`'s `else` branch: gate `view-import`'s visibility, the "Unpair
 * this device" visibility (keyed on `hasToken` alone — the help popover is
 * global, not scoped to any one phase), and reset the connected-only content
 * left over from a previous page. `onConnected` fires once per TRANSITION
 * into `connected` for the fire-and-forget auto-checks.
 */
const connectionStatus = mountConnectionStatus(els.connectionPillHost, els.connectionViewsHost, {
  send,
  onStatus: (status) => {
    els.unpairGroup.hidden = !status.hasToken;
    els.views.import.hidden = status.phase !== 'connected';
    if (status.phase !== 'connected') {
      els.jobCard.hidden = true;
      els.jobCardTitle.hidden = true;
      els.jobCardTitle.textContent = '';
      els.appliedStatus.hidden = true;
      els.appliedStatus.textContent = '';
      els.btnMarkApplied.hidden = true;
      els.btnMarkApplied.disabled = false;
      jobTools.reset();
      // The notice line is NOT cleared here: it reflects the shared per-tab
      // state, not a connection-scoped fetch — losing connection to the
      // desktop is not a reason to hide that the page already has answers.
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
 * Run the fire-and-forget `appliedCheck` and render its outcome into the
 * page-context card: title + a saved/applied chip (via
 * `job-status.ts`'s already-tested `resolveJobStatusView`, shared with the
 * panel's Job tab — this file has no local copy of that decision), plus the
 * adaptive import-button label and the "Mark as applied" button.
 * `runAppliedCheck` in background.ts already folds every failure mode into
 * `ok:true, result:{found:false}`, so the try/catch here only guards a
 * transport-level rejection (message-channel closed) — either way the card
 * just stays hidden.
 */
async function runAppliedAutoCheck(): Promise<void> {
  appliedCheckGeneration += 1;
  const myGeneration = appliedCheckGeneration;
  // Clear synchronously before the request goes out (belt-and-suspenders): if
  // render() re-enters `connected` for a new page while a previous check is
  // still in flight, the previous page's card must not linger while this
  // fresh one resolves.
  els.jobCard.hidden = true;
  els.jobCardTitle.hidden = true;
  els.jobCardTitle.textContent = '';
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
    const view = resolveJobStatusView(res);
    if (view) {
      els.jobCard.hidden = false;
      if (view.title) {
        els.jobCardTitle.textContent = view.title;
        els.jobCardTitle.hidden = false;
      }
      els.appliedStatus.textContent = view.chipText;
      els.appliedStatus.hidden = false;
    }
    jobTools.setImportLabel(resolveImportButtonLabel(res));
    // Only a found+saved result shows the button — reset disabled here too,
    // so a re-fire after a successful "Mark as applied" click (which left the
    // button disabled) ends re-enabled for whatever this fresh check renders.
    els.btnMarkApplied.hidden = !resolveShowMarkAppliedButton(res);
    els.btnMarkApplied.disabled = false;
  } catch {
    if (myGeneration !== appliedCheckGeneration) return;
    els.jobCard.hidden = true;
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
 * DOM update, so the chip flips to the applied wording and this button hides
 * itself once the fresh check confirms it.
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

// ── the "?" menu (PR0 §2: Help center / Settings / About) ──────────────────

type PopoverView = 'menu' | 'help' | 'about' | null;

let popoverView: PopoverView = null;

function setPopover(view: PopoverView): void {
  popoverView = view;
  els.menu.hidden = view !== 'menu';
  els.helpPopover.hidden = view !== 'help';
  els.aboutPopover.hidden = view !== 'about';
  els.btnHelp.setAttribute('aria-expanded', String(view !== null));
}

function toggleMenu(): void {
  setPopover(popoverView === null ? 'menu' : null);
}

function showAbout(): void {
  const version = browser.runtime.getManifest().version;
  els.aboutVersion.textContent = `AI Job Hunter — Job Importer v${version}`;
  setPopover('about');
}

function wire(): void {
  els.btnMarkApplied.addEventListener('click', () => void doMarkApplied());
  // NOT `void openAnswerPanel()` behind an await: opening the panel needs the
  // user gesture this click IS, and any await before the call spends it.
  els.btnOpenPanel.addEventListener('click', openAnswerPanel);
  els.btnUnpair.addEventListener('click', () => void unpair());
  els.btnHelp.addEventListener('click', toggleMenu);
  els.menuHelp.addEventListener('click', () => setPopover('help'));
  els.menuSettings.addEventListener('click', () => {
    setPopover(null);
    void browser.runtime.openOptionsPage();
  });
  els.menuAbout.addEventListener('click', showAbout);

  // The streamed draft itself is NOT rendered from this push: the background
  // mirrors every chunk into the shared per-tab state, and the panel's
  // Answers tab is subscribed to it — this popup only surfaces a TERMINAL
  // interruption on the shared status line, which the row itself cannot say
  // as loudly. (The `status` push is handled inside `connectionStatus` itself
  // — see its own `start()`.)
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
    if (res && res.ok && res.kind === 'answerAssistProgress' && res.done && res.interrupted) {
      setMsg(els.importMsg, 'Connection interrupted — here is what arrived so far.', 'err');
    }
  });
}

/**
 * Resolve the active tab id (for the panel-open call above) and subscribe the
 * notice line to this tab's shared answer state. Replaces the old Answer-tools
 * disclosure bootstrap now that the interactive rows live only in the panel.
 *
 * Exported (unlike the other `do*`/render helpers) because nothing wires a
 * user click to re-run this bootstrap — it only ever runs once, automatically,
 * at popup load — so it has no other seam for tests to drive it directly.
 */
export async function bootstrapNotice(): Promise<void> {
  try {
    const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
    activeTabId = typeof tab?.id === 'number' ? tab.id : null;
    if (activeTabId !== null) {
      subscribeAnswerState(activeTabId, (state) => {
        const line = resolveAnswersNoticeLine(state);
        els.answersNotice.hidden = line === null;
        els.answersNotice.textContent = line ?? '';
      });
    }
  } catch {
    // Best-effort — no tab id just means the notice line stays hidden.
  }
}

// `connectionStatus.start()` first: it registers the FIRST `onMessage`
// listener (the live status push), which `popup.test.ts` relies on finding
// at `mock.calls[0]` — `wire()`'s own `answerAssistProgress` listener must
// register second.
connectionStatus.start();
wire();
void bootstrapNotice();
