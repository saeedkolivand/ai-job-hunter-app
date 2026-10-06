/**
 * The right-click entries that open the answer panel (ADR-044 decision 2), and
 * the cross-browser "open the panel" call the fit badge's button shares.
 */

import { type Browser, browser } from '@wxt-dev/browser';

import { updateAnswerState, writeAnswerState } from '../lib/answer-state';
import { runAnswerAddRow } from './answer-rows';

/**
 * The selection-scoped entry (decision 2, amended — see
 * {@link ANSWER_PANEL_MENU_ID} for the second). Registered on `selection` only,
 * so it never appears on a page with nothing selected, and clicking it is
 * ITSELF the gesture that grants `activeTab` for that tab — one of the gestures
 * Chrome documents for `sidePanel.open`.
 */
const ANSWER_MENU_ID = 'ajh-answer-selection';

/**
 * The plain-right-click entry: opens the panel with nothing prefilled, so it has
 * no selection to require. Registered on `contexts: ['page', 'editable']`
 * rather than `'all'`, which would also fire on links/images/video — more than
 * a bare "open the panel" gesture needs.
 *
 * `'page'` is Chrome's LEAST-specific context: per Chromium's own matching rule
 * (`ExtensionContextAndPatternMatch`), a `page`-scoped item is suppressed
 * whenever a selection, link, editable field or media element is under the
 * cursor. So the two entries are mutually exclusive in the common case:
 *   - plain background, nothing selected: only this entry.
 *   - selected text, not editable: only `ANSWER_MENU_ID`.
 *   - inside an editable field, nothing selected: only this entry (`editable`
 *     is declared because the primary use is answering questions inside
 *     application-form fields, where a bare `page` context shows nothing).
 *   - inside an editable field WITH a selection: BOTH match — the one case
 *     where Chrome collapses multiple visible items into a single parent
 *     submenu titled with the manifest `name`
 *     (developer.chrome.com/docs/extensions/reference/api/contextMenus).
 */
const ANSWER_PANEL_MENU_ID = 'ajh-answer-open-panel';

/** Longest selection accepted as a question. A selection is untrusted page
 *  content; the desktop clamps it again, this just avoids carrying a whole
 *  article into the row list. */
const MAX_SELECTION_QUESTION = 500;

/** (Re)create the context-menu entries. `removeAll` first because `onInstalled`
 *  fires on every update and `create` throws on a duplicate id, which would
 *  otherwise poison the whole listener. */
export function installContextMenu(): void {
  const menus = browser.contextMenus;
  if (!menus) return;
  menus.removeAll(() => {
    menus.create({
      id: ANSWER_MENU_ID,
      title: 'Answer this with AI Job Hunter',
      contexts: ['selection'],
    });
    menus.create({
      id: ANSWER_PANEL_MENU_ID,
      title: 'Open AI Job Hunter answer tool',
      contexts: ['page', 'editable'],
    });
  });
}

/**
 * Origin from a tab's own `url`, captured DIRECTLY from the context-menu event's
 * `tab` — never a fresh `browser.tabs.query`, which could resolve to a DIFFERENT
 * tab if the user switched away during an intervening `await`. Missing or
 * malformed url → `''`, never a throw.
 */
function originFromTabUrl(url: string | undefined): string {
  try {
    return new URL(url ?? '').origin;
  } catch {
    return '';
  }
}

/**
 * Force `pageChanged: false` for `tabId`'s answer state ahead of a context-menu
 * gesture — a right-click IS a qualifying `activeTab` gesture, so both handlers
 * re-arm the panel's job-tools trust gate exactly like `runAnswerScan`, WITHOUT
 * scanning (a bare right-click implies nothing about wanting a re-scan).
 * `updateAnswerState` no-ops when no record exists, so a fresh tab still needs
 * the minimal record `runAnswerAddRow` builds — built here directly, because a
 * throwaway `runAnswerAddRow('', tabId)` would do nothing for an EXISTING record
 * (it never clears a stale `pageChanged`).
 *
 * `origin` is a PARAMETER, not re-derived: this awaits before it would need one,
 * and a fresh `activeTabOriginAtGesture()` could read a DIFFERENT tab's url if
 * the user switched tabs in the interim.
 */
async function rearmPageChangedForGesture(tabId: number, origin: string): Promise<void> {
  const updated = await updateAnswerState(tabId, (state) => ({ ...state, pageChanged: false }));
  if (updated) return;
  await writeAnswerState({
    tabId,
    origin,
    scannedAt: Date.now(),
    rows: [],
    stream: null,
    pageChanged: false,
  });
}

/**
 * Open the answer panel for `tabId` on whichever browser we are on. Chrome's
 * `sidePanel.open` and Firefox's `sidebarAction.open` BOTH require a user
 * gesture and are therefore called synchronously from a click handler, never
 * after an await. The panel's `default_path` is declared in the manifest, so
 * there is no `setOptions` call to lose the gesture on (design log 10a).
 */
export function openAnswerPanel(tabId: number | undefined): void {
  const chromePanel = (browser as { sidePanel?: { open(o: { tabId: number }): Promise<void> } })
    .sidePanel;
  if (chromePanel && typeof tabId === 'number') {
    void chromePanel.open({ tabId }).catch(() => {
      // A revoked gesture or a window that cannot host a panel — the popup's
      // own control is still there, so there is nothing to report here.
    });
    return;
  }
  const sidebar = (browser as { sidebarAction?: { open(): Promise<void> } }).sidebarAction;
  void sidebar?.open().catch(() => {
    // Same rationale as above.
  });
}

/**
 * Selection click: add the selection as a free-text row, then open the panel.
 * `open` is called from inside this handler because THIS click is the user
 * gesture — anything awaited before it loses the gesture, which is why the row
 * is added after the panel is opened rather than before.
 */
async function handleAnswerMenuClick(
  info: Browser.contextMenus.OnClickData,
  tab: Browser.tabs.Tab | undefined
): Promise<void> {
  const question = (info.selectionText ?? '').trim().slice(0, MAX_SELECTION_QUESTION);
  if (!question) return;
  openAnswerPanel(tab?.id);
  if (typeof tab?.id === 'number') {
    await rearmPageChangedForGesture(tab.id, originFromTabUrl(tab.url));
  }
  await runAnswerAddRow(question, tab?.id);
}

export function onContextMenuClicked(
  info: Browser.contextMenus.OnClickData,
  tab: Browser.tabs.Tab | undefined
): void {
  if (info.menuItemId === ANSWER_PANEL_MENU_ID) {
    openAnswerPanel(tab?.id);
    if (typeof tab?.id === 'number') {
      void rearmPageChangedForGesture(tab.id, originFromTabUrl(tab.url));
    }
    return;
  }
  if (info.menuItemId !== ANSWER_MENU_ID) return;
  void handleAnswerMenuClick(info, tab);
}
