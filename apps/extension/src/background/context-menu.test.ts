/** The context-menu entries that open the answer panel (ADR-044 decision 2). */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AnswerRow } from '../lib/answer-state';
import {
  activeTab,
  browser,
  contextMenuClick,
  executeScriptMock,
  flush,
  readAnswerState,
  resetMocks,
  send,
  stateOf,
  tabsQueryMock,
  writeAnswerState,
} from './test-support';

beforeEach(resetMocks);

const SELECTION_ENTRY = 'ajh-answer-selection';
const PANEL_ENTRY = 'ajh-answer-open-panel';

/** Fire a click on `menuItemId` for `tab` (what the browser hands the listener), then settle. */
async function click(menuItemId: string, tab: object, selectionText?: string): Promise<void> {
  contextMenuClick()({ menuItemId, selectionText } as never, tab as never);
  await flush();
}

const staleState = (tabId: number, rows: AnswerRow[]) => ({
  tabId,
  origin: 'https://jobs.example.com',
  scannedAt: 1,
  rows,
  stream: null,
  pageChanged: true,
});

describe('the context-menu entries (ADR-044 decision 2)', () => {
  it('registers both entries: the selection-only one and the plain-page one', () => {
    const onInstalled = vi.mocked(browser.runtime.onInstalled.addListener).mock.calls[0]?.[0];
    vi.mocked(browser.contextMenus.create).mockClear();

    onInstalled?.({} as never);

    expect(browser.contextMenus.create).toHaveBeenCalledTimes(2);
    expect(browser.contextMenus.create).toHaveBeenCalledWith(
      expect.objectContaining({
        id: SELECTION_ENTRY,
        title: 'Answer this with AI Job Hunter',
        contexts: ['selection'],
      })
    );
    expect(browser.contextMenus.create).toHaveBeenCalledWith(
      expect.objectContaining({
        id: PANEL_ENTRY,
        title: 'Open AI Job Hunter answer tool',
        contexts: ['page', 'editable'],
      })
    );
  });

  it('adds the trimmed selection as a free-text row on click, keyed to the clicked tab', async () => {
    activeTab('https://jobs.example.com/posting/8', 207);

    await click(SELECTION_ENTRY, { id: 207 }, '  Describe a challenge you solved.  ');

    // Read the resulting state back by asking for one more row.
    const { rows } = stateOf(await send({ kind: 'answerAddRow', question: 'a second question' }));
    expect(rows.map((r) => r.question)).toContain('Describe a challenge you solved.');
  });

  it('adds the row to the CLICKED tab even when a fresh active-tab query would resolve to a DIFFERENT tab (regression)', async () => {
    const clickedTabId = 208;
    const otherActiveTabId = 209;
    const question = 'Tell me about a conflict you resolved.';
    // Whichever tab a FRESH `{active:true,currentWindow:true}` query would
    // resolve to right now is a DIFFERENT tab than the one the context-menu event
    // fired on (e.g. focus moved to another window between the gesture and this
    // call) — the row must still land on the CLICKED tab.
    activeTab('https://jobs.example.com/posting/9', otherActiveTabId);

    await click(SELECTION_ENTRY, { id: clickedTabId }, question);

    expect((await readAnswerState(clickedTabId))?.rows.map((r) => r.question)).toContain(question);
    expect(
      (await readAnswerState(otherActiveTabId))?.rows.map((r) => r.question) ?? []
    ).not.toContain(question);
  });

  it('ignores a click on a different menu id', async () => {
    await click('some-other-entry', {}, 'ignored');

    expect(tabsQueryMock).not.toHaveBeenCalled();
  });

  it('opens the panel and adds no ROW on a plain-page click (no selection to prefill), but re-arms the trust gate for a tab with no prior state', async () => {
    const sidePanelOpenMock = vi.mocked(browser.sidePanel.open);
    sidePanelOpenMock.mockClear();
    const clickedTabId = 210;

    await click(PANEL_ENTRY, { id: clickedTabId, url: 'https://jobs.example.com/posting/11' });

    expect(sidePanelOpenMock).toHaveBeenCalledWith({ tabId: clickedTabId });
    // A plain right-click IS a qualifying `activeTab` gesture, so it writes the
    // SAME minimal record `runAnswerAddRow` builds for a fresh tab, with
    // `pageChanged` already armed `false`. The origin comes DIRECTLY from this
    // click's own `tab.url` — never a fresh `tabs.query`, which could resolve to a
    // DIFFERENT tab if the user switched away in the interim. No row is added —
    // a bare right-click implies nothing about wanting to re-scan the page.
    expect(tabsQueryMock).not.toHaveBeenCalled();
    expect(await readAnswerState(clickedTabId)).toEqual({
      tabId: clickedTabId,
      origin: 'https://jobs.example.com',
      scannedAt: expect.any(Number),
      rows: [],
      stream: null,
      pageChanged: false,
    });
  });

  it("degrades the new record's origin to '' when the clicked tab has no url, rather than writing something malformed", async () => {
    const clickedTabId = 216;

    // No `url` field at all on the tab object — same shape a restricted page
    // (chrome://, a PDF viewer) can hand a context-menu handler.
    await click(PANEL_ENTRY, { id: clickedTabId });

    // Never a fresh tab lookup either, for the same reason as the test above.
    expect(tabsQueryMock).not.toHaveBeenCalled();
    expect((await readAnswerState(clickedTabId))?.origin).toBe('');
  });

  it('clears a STALE pageChanged on the plain-page click without scanning or touching existing rows', async () => {
    const clickedTabId = 214;
    const existingRow: AnswerRow = {
      id: 'free:Existing question',
      question: 'Existing question',
      field: null,
      status: 'empty',
      versions: [],
      selected: -1,
    };
    await writeAnswerState(staleState(clickedTabId, [existingRow]));

    await click(PANEL_ENTRY, { id: clickedTabId });

    const state = await readAnswerState(clickedTabId);
    expect(state?.pageChanged).toBe(false);
    // No scan, no row mutation.
    expect(state?.rows).toEqual([existingRow]);
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('clears a STALE pageChanged on the selection click too, alongside adding the new row', async () => {
    const clickedTabId = 215;
    await writeAnswerState(staleState(clickedTabId, []));

    await click(SELECTION_ENTRY, { id: clickedTabId }, 'A fresh question.');

    const state = await readAnswerState(clickedTabId);
    expect(state?.pageChanged).toBe(false);
    expect(state?.rows.map((r) => r.question)).toContain('A fresh question.');
  });

  it('falls back to sidebarAction.open() on Firefox, which has no sidePanel API', async () => {
    // `openAnswerPanel` tries the Chrome `sidePanel` branch first and falls back
    // to Firefox's `sidebarAction` only when it is absent — simulate that by
    // removing `sidePanel` from the shared mock for this one test.
    const chromeSidePanel = (browser as { sidePanel?: unknown }).sidePanel;
    const sidebarOpenMock = vi.fn().mockResolvedValue(undefined);
    (browser as { sidePanel?: unknown }).sidePanel = undefined;
    (browser as { sidebarAction?: { open: typeof sidebarOpenMock } }).sidebarAction = {
      open: sidebarOpenMock,
    };
    try {
      await click(PANEL_ENTRY, { id: 212 });
      expect(sidebarOpenMock).toHaveBeenCalledTimes(1);
    } finally {
      (browser as { sidePanel?: unknown }).sidePanel = chromeSidePanel;
      delete (browser as { sidebarAction?: unknown }).sidebarAction;
    }
  });
});
