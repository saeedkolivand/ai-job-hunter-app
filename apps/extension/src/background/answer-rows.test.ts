/**
 * The shared per-(tab, origin) answer state (ADR-044), driven entirely through
 * the popup-request dispatcher: scan, free-text add, version select, and
 * Accept/Restore. Each test picks its OWN tabId (the mocked `storage.session` is
 * a module-level store, never reset between tests) so none reads another's state.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  activeTab,
  browser,
  executeScriptMock,
  flush,
  mockClient,
  paired,
  resetMocks,
  scanRows,
  scriptResults,
  send,
  stateOf,
  updateAnswerState,
} from './test-support';

beforeEach(resetMocks);

const WHY = { question: 'Why this role?', index: 0 };

describe('answerScan request (ADR-044)', () => {
  it('injects capture-rows.js, captures the origin at gesture time, and writes the built state', async () => {
    activeTab('https://jobs.example.com/posting/1', 200);
    scriptResults({
      questions: [WHY],
      filled: [{ question: 'Company name', index: 0, answer: 'Acme' }],
    });
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });

    const state = stateOf(await send({ kind: 'answerScan' }));

    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 200 },
      files: ['capture-rows.js'],
    });
    expect(state.tabId).toBe(200);
    expect(state.origin).toBe('https://jobs.example.com');
    expect(state.pageChanged).toBe(false);
    expect(state.rows.map((r) => r.question)).toEqual(['Why this role?', 'Company name']);
  });

  it('surfaces "Could not read the questions on this page." when the injected script returns a non-scan value', async () => {
    activeTab('https://jobs.example.com/posting/2', 201);
    scriptResults(null);

    const res = await send({ kind: 'answerScan' });

    expect(res).toEqual({ ok: false, error: 'Could not read the questions on this page.' });
  });
});

describe('answerAddRow request (ADR-044)', () => {
  it('creates a fresh state (unscanned page) carrying only the free-text row', async () => {
    activeTab('https://jobs.example.com/posting/3', 202);

    const { rows } = stateOf(
      await send({ kind: 'answerAddRow', question: 'What is your visa status?' })
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      question: 'What is your visa status?',
      field: null,
      status: 'empty',
    });
  });

  it('prepends onto an existing scan rather than replacing it, and reuses the row on a repeated question', async () => {
    await scanRows(203, 'Why this role?');

    const first = stateOf(
      await send({ kind: 'answerAddRow', question: 'A question the scan missed' })
    );
    const second = stateOf(
      await send({ kind: 'answerAddRow', question: 'A question the scan missed' })
    );

    const expected = ['A question the scan missed', 'Why this role?'];
    expect(first.rows.map((r) => r.question)).toEqual(expected);
    // Same question added twice reuses the row rather than stacking a duplicate.
    expect(second.rows.map((r) => r.question)).toEqual(expected);
  });
});

describe('answerSelectVersion request (ADR-044)', () => {
  it('selects a version by index, and falls back to -1 (the page text) for an out-of-range index', async () => {
    const [rowId] = (await scanRows(204, 'Why this role?')) as [string];

    // Seed a version to select — a fresh row has none, so `version: 0` would
    // ALSO fall back to -1, and the test would never actually exercise the
    // in-range branch it claims to cover.
    await updateAnswerState(204, (state) => ({
      ...state,
      rows: state.rows.map((row) =>
        row.id === rowId
          ? { ...row, versions: [{ label: 'v1', text: 'A drafted answer.', kind: 'draft' }] }
          : row
      ),
    }));

    const inRange = stateOf(await send({ kind: 'answerSelectVersion', rowId, version: 0 }));
    expect(inRange.rows[0]?.selected).toBe(0);

    const outOfRange = stateOf(await send({ kind: 'answerSelectVersion', rowId, version: 5 }));
    expect(outOfRange.rows[0]?.selected).toBe(-1);
  });
});

describe('answerAccept / answerRestoreOriginal requests (ADR-044)', () => {
  const accepted = { ok: true, kind: 'answerAccept', result: { filled: true } };

  it('writes the selected text into an EMPTY field via answer-fill.js and remembers it as currentText', async () => {
    paired();
    const [rowId] = (await scanRows(205, 'Why this role?')) as [string];
    scriptResults(undefined, { filled: true }); // answer-fill.js registration, then the call

    // A freshly-scanned row has no drafted version yet, so Restore (which always
    // has text — the frozen scan-time original, `''` for an empty field) is what
    // exercises `writeRowText`'s fail-safe write path here; Accept goes through
    // the identical function with a different source text.
    const res = await send({ kind: 'answerRestoreOriginal', rowId });

    // Call 1 was the scan's own capture-rows.js injection; 2 and 3 are this write.
    expect(executeScriptMock).toHaveBeenNthCalledWith(2, {
      target: { tabId: 205 },
      files: ['answer-fill.js'],
    });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        target: { tabId: 205 },
        args: ['Why this role?', 0, 1, '', '__ajhRunAnswerFill'],
      })
    );
    expect(res).toEqual(accepted);
  });

  // #1230: a successful write into an EMPTY field flips `field.kind` to
  // 'filled'. Without that flip the SECOND write still takes the fill path,
  // whose locator only searches EMPTY candidates — so it cannot find the field
  // it just filled and returns the fixed NOT_FOUND ("the page may have
  // changed"), which is false: the field is right there, holding our own text.
  it('takes the REPLACE path on a second write, because the first flipped the field to filled', async () => {
    paired();
    const [rowId] = (await scanRows(207, 'Why this role?')) as [string];

    // First write — the field is still empty, so this is the fill path.
    scriptResults(undefined, { filled: true });
    expect(await send({ kind: 'answerRestoreOriginal', rowId })).toEqual(accepted);
    expect(executeScriptMock).toHaveBeenNthCalledWith(2, {
      target: { tabId: 207 },
      files: ['answer-fill.js'],
    });

    // Second write on the SAME row — the field is now genuinely filled, so it
    // must go through answer-replace.js, carrying the previous text as the
    // expected current value.
    scriptResults(undefined, { filled: true });
    expect(await send({ kind: 'answerRestoreOriginal', rowId })).toEqual(accepted);
    expect(executeScriptMock).toHaveBeenNthCalledWith(4, {
      target: { tabId: 207 },
      files: ['answer-replace.js'],
    });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      5,
      expect.objectContaining({
        target: { tabId: 207 },
        args: ['Why this role?', 0, 1, '', '', '__ajhRunAnswerReplace'],
      })
    );
  });

  it('refuses to write once the page has changed, without touching the tab at all', async () => {
    paired();
    const [rowId] = (await scanRows(206, 'Why this role?')) as [string];

    // A navigation flips pageChanged — the onUpdated listener does this in the
    // real worker; call the registered callback directly the same way the
    // onMessage listener is driven.
    const onUpdated = vi.mocked(browser.tabs.onUpdated.addListener).mock.calls[0]?.[0];
    onUpdated?.(206, { status: 'loading' } as never, {} as never);
    await flush();

    executeScriptMock.mockClear();
    const res = await send({ kind: 'answerRestoreOriginal', rowId });

    expect(res).toEqual({
      ok: false,
      error: 'This page changed. Click the toolbar icon to scan it, then try again.',
    });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});
