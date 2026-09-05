/**
 * Side-panel controller (Chrome `sidePanel`, Firefox `sidebar_action`).
 *
 * It is deliberately thin. The panel is the SECOND view of ADR-044 decision
 * 1's one state, not a second implementation of the Answer tools: it mounts
 * the same component the popup mounts, against the same `storage.session`
 * record, and adds only the two things that are true of a panel and not of a
 * popup.
 *
 * 1. **It outlives the click that uses it.** That is the whole point — a
 *    copy-only tool disappearing on blur is the defect this record answers.
 *    Nothing here has to be done to get that; it is what a panel is.
 * 2. **It is per WINDOW, not per tab.** Chrome keeps one panel open across
 *    tab switches, so it follows `tabs.onActivated` and re-subscribes to
 *    whichever tab is now active. A tab's ID is readable without the `tabs`
 *    permission (only its url and title are gated behind it), which is what
 *    lets this work while `tabs` stays on the manifest denylist.
 *
 * The panel has NO page access of its own and never asks for any: every read
 * or write of the page goes through the background, which acts under the
 * `activeTab` grant a user gesture created. After a navigation the shared
 * state says so and the component replaces every write control with one line
 * — this file does not need to know about that case at all. The same is now
 * true of the job-tools controls (Import/Check fit/Fill/Save answers) mounted
 * below — see `job-tools.ts`'s doc for its own trust gate, which this file
 * only has to feed via `jobTools.render`/`jobTools.checkPage`.
 */

import { browser } from '@wxt-dev/browser';

import { copyText, mountAnswerTools } from '../answer-tools/answer-tools';
import { mountJobTools } from '../job-tools/job-tools';
import { subscribeAnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

// The panel loads the POPUP's stylesheet, not a copy of it. A forked theme is
// how two surfaces start looking like two products.
import '../popup/popup.css';

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

/** Send a typed request to the background — the same seam the popup uses. */
async function send(req: PopupRequest): Promise<PopupResponse> {
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

const answerTools = mountAnswerTools(byId<HTMLDivElement>('answer-tools-host'), {
  send,
  copy: copyText,
});

// No `onAnswerToolsVisibility` here: the panel's Answer-tools section has no
// disclosure to gate on the fields probe today (unlike the popup's), and
// adding that is out of scope for this parity change.
const jobTools = mountJobTools(byId<HTMLDivElement>('job-tools-host'), { send });

/** Unsubscribe the previous tab's state subscription, if any. */
let unsubscribe: (() => void) | null = null;

/**
 * Point the panel at `tabId`'s state. Dropping the previous subscription
 * first is load-bearing: a panel that accumulated one listener per tab switch
 * would keep re-rendering with a background tab's rows on top of the active
 * one's, which is exactly the confusion a per-window surface has to avoid.
 *
 * `jobTools.checkPage()` fires from INSIDE the subscription's own first
 * delivery, never as a separate statement right after `subscribeAnswerState`
 * — that read is unavoidably asynchronous (`readAnswerState(tabId).then(...)`
 * in `lib/answer-state.ts`), so a `checkPage()` call placed right here would
 * run against whatever `jobTools`'s trust flag was left over from BEFORE this
 * follow (the previous tab, or its cold-mount default) and could fire the
 * fields probe against a tab job-tools has not actually evaluated yet — the
 * exact ungated call its own trust gate exists to prevent. Waiting for the
 * first delivery means `jobTools.render(state)` has already updated that flag
 * to the tab actually being followed by the time `checkPage()` reads it. This
 * function is still both of the job-tools trigger points its own doc names
 * for the panel ("mount and tab activation"): the first call from
 * `resolvePanelWindowId().then(...)` below is the mount, every later call
 * from `tabs.onActivated`/a focus change is an activation — `firstDelivery`
 * just defers each one's `checkPage()` to the moment it is actually safe to
 * fire, without changing which gesture triggers it.
 */
function follow(tabId: number | null): void {
  unsubscribe?.();
  unsubscribe = null;
  if (tabId === null) {
    answerTools.render(null);
    jobTools.render(null);
    return;
  }
  let firstDelivery = true;
  unsubscribe = subscribeAnswerState(tabId, (state) => {
    answerTools.render(state);
    jobTools.render(state);
    if (firstDelivery) {
      firstDelivery = false;
      jobTools.checkPage();
    }
  });
}

/** This panel's own window — resolved once, since a panel never migrates
 *  windows. Every tab lookup below is pinned to it, so an activation or focus
 *  change in an UNRELATED window can never hijack this panel's subscription. */
let panelWindowId: number | null = null;

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
  void activeTabId().then(follow);
});
