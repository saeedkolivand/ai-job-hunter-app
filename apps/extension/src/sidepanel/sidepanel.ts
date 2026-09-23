/**
 * Side-panel controller (Chrome `sidePanel`, Firefox `sidebar_action`).
 *
 * The panel is the WORKSPACE surface of the PR0 redesign (`.claude/scratch/
 * extension-round-design.md`): a tab bar (`src/tabs/tabs.ts`) with a Job tab
 * (page-context card + stage strip + the four job-tools controls) and an
 * Answers tab (the existing Answer-tools component, restyled but functionally
 * unchanged). It is deliberately thin — the panel is the SECOND view of
 * ADR-044 decision 1's one Answer-tools state, not a second implementation,
 * and the job-status card is likewise a pure view over `appliedCheck`
 * (`job-status.ts`'s own doc).
 *
 * 1. **It outlives the click that uses it.** That is the whole point — a
 *    copy-only tool disappearing on blur is the defect ADR-044 answers.
 *    Nothing here has to be done to get that; it is what a panel is.
 * 2. **It is per WINDOW, not per tab.** Chrome keeps one panel open across
 *    tab switches, so it follows `tabs.onActivated` and re-subscribes to
 *    whichever tab is now active. A tab's ID is readable without the `tabs`
 *    permission (only its url and title are gated behind it), which is what
 *    lets this work while `tabs` stays on the manifest denylist.
 * 3. **It has connection-status awareness (ADR-046)**: the shared
 *    `connection-status.ts` module (also mounted by the popup) owns the
 *    pill/retry + pair/offline/outdated/searching views, and this file's only
 *    job is to show/hide `#view-connected` based on the phase it reports.
 *
 * The panel has NO page access of its own and never asks for any: every read
 * or write of the page goes through the background, which acts under the
 * `activeTab` grant a user gesture created. After a navigation the shared
 * state says so and job-tools replaces its own write controls with one line
 * — see job-tools.ts's own trust-gate doc, which this file only feeds via
 * `jobTools.render`/`jobTools.checkPage`.
 */

import { type Browser, browser } from '@wxt-dev/browser';

import { copyText, mountAnswerTools } from '../answer-tools/answer-tools';
import { mountConnectionStatus } from '../connection-status/connection-status';
import { mountDocuments } from '../documents/documents';
import { mountJobStatus } from '../job-status/job-status';
import { isPageTrusted, JOB_TOOLS_GATED_LINE, mountJobTools } from '../job-tools/job-tools';
import { type AnswerState, subscribeAnswerState } from '../lib/answer-state';
import { getDefaultPanelTab } from '../lib/appearance';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import {
  getRememberedHosts,
  hostOf,
  mountFirstFillConfirm,
  rememberHost,
} from '../lib/site-memory';
import { bootTheme } from '../lib/theme';
import { mountPrep } from '../prep/prep';
import { mountTabs } from '../tabs/tabs';

/** The résumé-attach confirmation's own copy (PR2 §C.3) — reuses the SAME
 *  first-time-per-site inset the Fill button already shows (R6 of the
 *  redesign record), with this gesture's own text. */
const ATTACH_CONFIRM_COPY = 'This will attach your résumé file — nothing is submitted.';

// The panel loads the POPUP's stylesheet, not a copy of it. A forked theme is
// how two surfaces start looking like two products.
import '../popup/popup.css';

// Apply the Settings → Appearance → Theme choice before anything else renders.
void bootTheme();

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

const els = {
  viewConnected: byId<HTMLElement>('view-connected'),
  connectionPillHost: byId<HTMLDivElement>('connection-pill-host'),
  connectionViewsHost: byId<HTMLDivElement>('connection-views-host'),
  btnSettings: byId<HTMLButtonElement>('btn-settings'),
  trustLine: byId<HTMLParagraphElement>('trust-line'),
  tabsHost: byId<HTMLDivElement>('tabs-host'),
  autoSaveNotice: byId<HTMLDivElement>('auto-save-notice'),
  autoSaveNoticeText: byId<HTMLParagraphElement>('auto-save-notice-text'),
  autoSaveNoticeDismiss: byId<HTMLButtonElement>('auto-save-notice-dismiss'),
};

/** This panel's own window — resolved once, since a panel never migrates
 *  windows. Every tab lookup below is pinned to it, so an activation or focus
 *  change in an UNRELATED window can never hijack this panel's subscription.
 *
 *  Declared HERE, above {@link send}: `checkAutoSaveNotice()` sends at module
 *  load, long before the resolver below runs, and a `let` read before its own
 *  declaration is evaluated throws (temporal dead zone) — which that function's
 *  catch would swallow, silently killing the notice. */
let panelWindowId: number | null = null;

/**
 * Send a typed request to the background — the same seam the popup uses.
 *
 * Every request carries this panel's own {@link panelWindowId} so the
 * background targets THIS window's active tab. Its `currentWindow: true`
 * fallback resolves to the last-focused window, which is a different window
 * whenever the user has focused another one — the mistarget behind #1215. The
 * id is resolved once during `init()`; a request issued before that (or in the
 * rare case the lookup failed) simply omits it and keeps the old behaviour.
 */
async function send(req: PopupRequest): Promise<PopupResponse> {
  if (panelWindowId !== null) req = { ...req, windowId: panelWindowId };
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

els.btnSettings.addEventListener('click', () => {
  void browser.runtime.openOptionsPage();
});

els.autoSaveNoticeDismiss.addEventListener('click', () => {
  els.autoSaveNotice.hidden = true;
});

/**
 * The one-shot save-answers-on-submit auto-save notice (PR4, decision 7 —
 * "the user must never discover this silently"). Read-once, same discipline
 * as `popup.ts`'s own `checkAutoSaveNotice` — whichever surface asks first
 * gets it. Local session storage, not a bridge call, so it fires
 * unconditionally at load, needing no connection.
 */
async function checkAutoSaveNotice(): Promise<void> {
  try {
    const res = await send({ kind: 'autoSaveNotice' });
    if (res.ok && res.kind === 'autoSaveNotice' && res.text) {
      els.autoSaveNoticeText.textContent = res.text;
      els.autoSaveNotice.hidden = false;
    }
  } catch {
    // Best-effort — a missed notice this once is better than a broken panel.
  }
}
void checkAutoSaveNotice();

// ── the tab bar (PR0 §3, PR4 adds Prep) ─────────────────────────────────────

const tabs = mountTabs(
  els.tabsHost,
  [
    { id: 'job', label: 'Job' },
    { id: 'documents', label: 'Documents' },
    { id: 'answers', label: 'Answers' },
    { id: 'prep', label: 'Prep' },
  ],
  { onSelect: (id) => selectTab(id) }
);
// Job is the default active tab — set synchronously so a panel is visible
// immediately, before the persisted per-window preference (async, and best-
// effort if `storage.session` is unavailable) can override it below.
tabs.setActive('job');

const jobPanel = tabs.panel('job');
const jobHeader = document.createElement('div');
jobHeader.className = 'job-header';
// A button, not an `<a href="ajh://…">` — this extension's own deep links
// (connection-status.ts's PAIRING_DEEP_LINK/GET_APP_URL, options.ts) all fire
// through `browser.tabs.create` in a click handler; a raw custom-scheme
// anchor is unverified cross-browser and can silently no-op if the click
// falls through without a handler.
const openInAppLink = document.createElement('button');
openInAppLink.type = 'button';
openInAppLink.className = 'btn btn--quiet';
openInAppLink.textContent = 'Open in app';
openInAppLink.hidden = true;
let openInAppUrl = '';
async function openInApp(): Promise<void> {
  try {
    await browser.tabs.create({ url: openInAppUrl });
  } catch {
    // No-op: the deep link is best-effort — same discipline as
    // connection-status.ts's own deep links.
  }
}
openInAppLink.addEventListener('click', () => void openInApp());
jobHeader.append(openInAppLink);
jobPanel.append(jobHeader);
const jobStatusHost = document.createElement('div');
jobPanel.append(jobStatusHost);
const jobToolsHost = document.createElement('div');
jobToolsHost.id = 'job-tools-host';
jobPanel.append(jobToolsHost);

const documentsPanel = tabs.panel('documents');
const documentsHost = document.createElement('div');
documentsHost.id = 'documents-host';
documentsPanel.append(documentsHost);

const answersPanel = tabs.panel('answers');
const answerToolsHost = document.createElement('div');
answerToolsHost.id = 'answer-tools-host';
answerToolsHost.className = 'atools';
answersPanel.append(answerToolsHost);

const prepPanel = tabs.panel('prep');
const prepHost = document.createElement('div');
prepHost.id = 'prep-host';
prepPanel.append(prepHost);

// First-time Fill confirmation (PR0 §4) — mounted ONCE into #view-connected
// (a SIBLING of the tab bar, OUTSIDE every `[data-section]` panel — #1224:
// inside a tab panel the inset was hidden by `tabs.ts`'s `setActive`
// whenever that tab wasn't active, so from the Documents tab the Attach
// confirmation was invisible and its promise never resolved). Appended AFTER
// `mountTabs`, so it sits below the tab content and can never intercept a
// tab click. Fed the CURRENTLY-followed tab's origin (updated on every
// state push below).
const fillConfirmHost = document.createElement('div');
els.viewConnected.append(fillConfirmHost);
const fillConfirm = mountFirstFillConfirm(fillConfirmHost, { getRememberedHosts, rememberHost });

let currentOrigin: string | null = null;

/**
 * Generation guard against a STALE in-flight subscription callback — mirrors
 * `popup.ts`'s `appliedCheckGeneration`/`fieldsProbeGeneration` pattern
 * exactly (same stale-response race those already guard against).
 * `subscribeAnswerState`'s returned unsubscribe (called at the top of
 * `follow` below) only removes the `storage.onChanged` listener — it does
 * NOT cancel the in-flight `readAnswerState(tabId).then(onState)` promise
 * the SAME call already kicked off (`lib/answer-state.ts`). So a rapid
 * `follow(A)` → `follow(B)` (fast tab-cycling) can still let A's stale
 * closure fire its `onState` callback AFTER B's own subscription — and
 * possibly after B's own render — with A's now-irrelevant data. Declared
 * ahead of `jobTools`'s own mount below since its `confirmFill` dep already
 * needs to snapshot this value.
 */
let followGeneration = 0;

const answerTools = mountAnswerTools(answerToolsHost, { send, copy: copyText });

const jobStatus = mountJobStatus(jobStatusHost, { send });

// No `onAnswerToolsVisibility` here: the panel's Answer-tools section has no
// disclosure to gate on the fields probe today (unlike the popup's), and
// adding that is out of scope for this parity change. `hideSaveAnswers` is
// also omitted — the panel keeps all four job-tools controls.
const jobTools = mountJobTools(jobToolsHost, {
  send,
  // Snapshot the origin + follow generation at the moment the confirmation
  // opens. `follow()` cancels an open inset immediately on a target change
  // (hiding it + resolving `false`), but the panel can still switch targets
  // in the gap between the user's own click and this async confirm()
  // settling — re-validate both before treating the click as authorized for
  // THIS page.
  // TODO(follow-up, out of scope for this PR): thread the captured/validated
  // tab id through the `fill` request itself (background.ts) rather than
  // relying solely on this caller-side re-check.
  confirmFill: async () => {
    const capturedGeneration = followGeneration;
    const capturedOrigin = currentOrigin;
    const ok = await fillConfirm.confirm(hostOf(capturedOrigin));
    if (!ok) return false;
    return followGeneration === capturedGeneration && currentOrigin === capturedOrigin;
  },
});

// Documents tab (PR2) — reuses the SAME first-time-per-site confirmation as
// Fill above (R6 "reuses the R6 Fill confirmation" — one inset, one
// remembered-host set, a different copy per gesture).
const documents = mountDocuments(documentsHost, {
  send,
  copy: copyText,
  confirmAttach: (host) => fillConfirm.confirm(host, ATTACH_CONFIRM_COPY, 'Attach'),
  currentHost: () => hostOf(currentOrigin),
  getFollowGeneration: () => followGeneration,
  onUrlResolved: (url) => {
    openInAppUrl = `ajh://open?url=${encodeURIComponent(url)}`;
    openInAppLink.hidden = false;
  },
});

// Prep tab (PR4) — reads this job's existing generations through the read
// tier; the two on-demand drafts ride the SAME `answer.assist` stream
// Answer-tools rows use, correlated by `topic` instead of `rowId`.
const prep = mountPrep(prepHost, { send, copy: copyText });

/**
 * The one trust line under the header (ADR-045 of the redesign record). Only
 * the TRUSTED case renders here — synchronously the host-only fallback
 * ("Reading: <host>", derived from `AnswerState.origin`), immediately
 * upgraded to "Reading: <title> · <company>" if/when {@link
 * refreshTrustLineJob} answers (PR1 — extension read tier); NEVER blocks on
 * that query. The untrusted case is intentionally left to job-tools.ts's own
 * `JOB_TOOLS_GATED_LINE` (rendered inside the Job tab, already covered by its
 * own tests) — showing the identical sentence in both places would be a
 * literal duplicate on screen, which is worse than the mockup's exact
 * position.
 */
function updateTrustLine(state: AnswerState | null): void {
  if (state && isPageTrusted(state)) {
    let host = state.origin;
    try {
      host = new URL(state.origin).hostname;
    } catch {
      // Keep the raw origin — better than nothing.
    }
    els.trustLine.textContent = `Reading: ${host}`;
    els.trustLine.hidden = false;
  } else {
    els.trustLine.textContent = '';
    els.trustLine.hidden = true;
  }
}

/**
 * Generation guard against a STALE in-flight `trustLineJob` query
 * superseding a newer one — mirrors `followGeneration`/`lastJobStatusKey`'s
 * own staleness discipline. Bumped by every call (so a fast tab-cycle's
 * earlier query can never overwrite a later one's host-only fallback) and
 * by `follow()`'s reset paths, so a query started for a page the panel has
 * since left can never write into the current trust line.
 */
let trustLineJobGeneration = 0;

/**
 * Upgrade the already-rendered host-only trust line to "Reading: <title> ·
 * <company>" once the read tier answers (PR1). Called ONLY for a trusted
 * page, alongside `jobStatus.refresh()`, from the SAME trust-relevant-bits
 * de-dup `follow()` already applies — never on every streamed chunk. ANY
 * refusal (Autofill off, throttled, an unknown job) or a request failure
 * simply leaves the synchronous host-only line in place — this never blocks
 * or errors the panel.
 */
async function refreshTrustLineJob(): Promise<void> {
  trustLineJobGeneration += 1;
  const myGeneration = trustLineJobGeneration;
  try {
    const res = await send({ kind: 'trustLineJob' });
    if (myGeneration !== trustLineJobGeneration) return; // a newer call/reset superseded this one
    if (res.ok && res.kind === 'trustLineJob' && res.title) {
      els.trustLine.textContent = res.company
        ? `Reading: ${res.title} · ${res.company}`
        : `Reading: ${res.title}`;
    }
  } catch {
    // Best-effort — keep whatever `updateTrustLine` already rendered.
  }
}

/**
 * The connection-status pill/retry + the four non-connected views — the SAME
 * component the popup mounts (ADR-046). `onStatus` fires on every render and
 * is the panel's ONLY connection-status responsibility: show `view-connected`
 * (the trust line + tabs) only while `phase === 'connected'`, the
 * non-connected view otherwise. Unlike the popup, the panel needs no
 * `onConnected`/`onPaired` — its own tab-follow logic (`follow()` below)
 * already runs `jobTools.checkPage()` independently on mount/tab-activation,
 * and it has no focus target to move on a fresh pair.
 */
mountConnectionStatus(els.connectionPillHost, els.connectionViewsHost, {
  send,
  onStatus: (status) => {
    els.viewConnected.hidden = status.phase !== 'connected';
  },
}).start();

/** Unsubscribe the previous tab's state subscription, if any. */
let unsubscribe: (() => void) | null = null;

/** The (origin, pageChanged) pair `jobStatus` last refreshed/reset for —
 *  reset to `null` on every `follow()` call so a freshly-followed tab always
 *  gets its own first check, and updated only when a state push actually
 *  changes it. Without this, every streamed answer-state update (each
 *  chunk changes only `stream`/`rows`, not `origin`/`pageChanged`) re-ran
 *  `appliedCheck` — `job-status.ts`'s own generation guard already covers
 *  the staleness risk of the calls this now makes, so nothing extra is
 *  needed here for that. */
let lastJobStatusKey: string | null = null;

function jobStatusKeyOf(state: AnswerState | null): string {
  return state ? `${state.origin}|${state.pageChanged}` : 'none';
}

/**
 * Point the panel at `tabId`'s state. Dropping the previous subscription
 * first is load-bearing: a panel that accumulated one listener per tab switch
 * would keep re-rendering with a background tab's rows on top of the active
 * one's, which is exactly the confusion a per-window surface has to avoid.
 *
 * `jobTools.checkPage()` (and `jobStatus.refresh()`, its equivalent for the
 * page-context card) fire from INSIDE the subscription's own first delivery,
 * never as a separate statement right after `subscribeAnswerState` — see
 * job-tools.ts's `checkPage` doc for why that ordering is load-bearing.
 */
function follow(tabId: number | null): void {
  followGeneration += 1;
  const myGeneration = followGeneration;
  unsubscribe?.();
  unsubscribe = null;
  // Any confirmation left open belonged to the target `follow()` is about to
  // leave — cancel it (hides the inset, resolves its promise `false`) rather
  // than let it linger over whatever this call is about to show instead.
  fillConfirm.cancel();
  lastJobStatusKey = null;
  trustLineJobGeneration += 1; // invalidate any in-flight query for the tab being left
  if (tabId === null) {
    answerTools.render(null);
    jobTools.render(null);
    documents.render(null);
    documents.reset(JOB_TOOLS_GATED_LINE);
    prep.render(null);
    prep.reset(JOB_TOOLS_GATED_LINE);
    openInAppLink.hidden = true;
    jobStatus.reset();
    updateTrustLine(null);
    currentOrigin = null;
    return;
  }
  let firstDelivery = true;
  unsubscribe = subscribeAnswerState(tabId, (state) => {
    // A newer `follow()` call has since superseded this one — its render (or
    // its own in-flight read) must win; bail before touching anything.
    if (myGeneration !== followGeneration) return;
    answerTools.render(state);
    jobTools.render(state);
    documents.render(state);
    prep.render(state);
    updateTrustLine(state);
    currentOrigin = state?.origin ?? null;
    tabs.setCount('answers', state?.rows.length ?? 0);
    // Only re-check job-status when the trust-relevant bits of the state
    // actually changed — a streamed answer update pushes a fresh `state` on
    // every chunk without touching either.
    const key = jobStatusKeyOf(state);
    if (key !== lastJobStatusKey) {
      lastJobStatusKey = key;
      if (state && isPageTrusted(state)) {
        void jobStatus.refresh();
        void refreshTrustLineJob();
        documents.refresh();
        prep.refresh();
      } else {
        jobStatus.reset();
        trustLineJobGeneration += 1; // invalidate any in-flight query — the page is no longer trusted
        documents.reset(JOB_TOOLS_GATED_LINE);
        prep.reset(JOB_TOOLS_GATED_LINE);
        openInAppLink.hidden = true;
      }
    }
    if (firstDelivery) {
      firstDelivery = false;
      jobTools.checkPage();
    }
  });
}

// ── active tab, remembered per browser window (PR0 §3) ──────────────────────

/** `storage.session` key for the last active tab of `windowId`. */
function activeTabKey(windowId: number): string {
  return `sidepanelActiveTab:${windowId}`;
}

/** `storage.session` handle — absent on an engine/context without it, in
 *  which case the active tab just defaults to `job` every time (same
 *  best-effort discipline as `lib/answer-state.ts`'s `sessionArea`). */
function sessionArea(): Browser.storage.StorageArea | null {
  const area = (browser.storage as { session?: Browser.storage.StorageArea }).session;
  return area ?? null;
}

function selectTab(id: string): void {
  tabs.setActive(id);
  if (panelWindowId === null) return;
  const area = sessionArea();
  if (!area) return;
  void area.set({ [activeTabKey(panelWindowId)]: id }).catch(() => undefined);
}

/** No per-window tab was ever persisted (or `storage.session` is
 *  unavailable/erroring) — fall back to the Settings → Appearance → Default
 *  panel tab choice rather than a hardcoded 'job', same discipline as every
 *  other best-effort read in this file. */
async function loadActiveTab(windowId: number): Promise<string> {
  const area = sessionArea();
  if (!area) return getDefaultPanelTab();
  try {
    const stored = await area.get(activeTabKey(windowId));
    const value = stored[activeTabKey(windowId)];
    // 'documents'/'prep' (PR2/PR4) are valid per-window session choices even
    // though neither is (yet) a `DefaultPanelTab` the Settings page can
    // target — see that type's own doc for why the two are a deliberately
    // narrower/wider pair.
    if (value === 'answers' || value === 'job' || value === 'documents' || value === 'prep') {
      return value;
    }
    return getDefaultPanelTab();
  } catch {
    return getDefaultPanelTab();
  }
}

async function resolvePanelWindowId(): Promise<number | null> {
  try {
    const win = await browser.windows.getCurrent();
    return typeof win.id === 'number' ? win.id : null;
  } catch {
    return null;
  }
}

/** The active tab of the window this panel belongs to. */
async function activeTabId(): Promise<number | null> {
  if (panelWindowId === null) return null;
  try {
    const [tab] = await browser.tabs.query({ active: true, windowId: panelWindowId });
    return typeof tab?.id === 'number' ? tab.id : null;
  } catch {
    return null;
  }
}

browser.tabs.onActivated.addListener((info) => {
  if (info.windowId !== panelWindowId) return;
  follow(info.tabId);
});

// A window focus change can flip which tab is "active" in THIS panel's own
// window (e.g. a tab activated there while unfocused) — re-resolve, still
// scoped to `panelWindowId`, never to whichever window just gained focus.
browser.windows?.onFocusChanged.addListener(() => {
  void activeTabId().then(follow);
});

void resolvePanelWindowId().then((id) => {
  panelWindowId = id;
  if (id !== null) void loadActiveTab(id).then((tabId) => tabs.setActive(tabId));
  void activeTabId().then(follow);
});
