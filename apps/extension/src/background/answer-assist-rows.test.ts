/**
 * `answerAssist` settling into its answer ROW: the unchanged-rewrite no-op guard,
 * and the two races that could clobber the row (`updateAnswerState`'s per-tab
 * write queue, and a late `activeTabId()` re-arming a superseded run).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  assistOk,
  browser,
  flush,
  mockClient,
  paired,
  readAnswerState,
  resetMocks,
  scanRows,
  send,
  stateOf,
  tabsQueryMock,
} from './test-support';

beforeEach(async () => {
  await resetMocks();
  paired();
});

const draftRow = (rowId: string, extra: Record<string, unknown> = {}) =>
  send({
    kind: 'answerAssist',
    question: 'Why this role?',
    searchWeb: false,
    mode: 'draft',
    rowId,
    ...extra,
  } as never);

describe('settleRowFromAssist — a chip rewrite that comes back unchanged', () => {
  const DRAFT = 'Led the migration and shipped the payment service.';

  /** Scan one row and draft it once, so there is a version a rewrite can reshape
   *  (`runAnswerRowAssist` refuses a rewrite with nothing to reshape yet). */
  async function scanAndDraft(tabId: number): Promise<string> {
    const [rowId] = await scanRows(tabId, 'Why this role?');
    mockClient.answerAssist.mockResolvedValueOnce(assistOk(DRAFT));
    await draftRow(rowId as string);
    return rowId as string;
  }

  /** Rewrite the row with `draft` as the desktop's reply and read the rows back —
   *  `version: 0` is a no-op re-select of the version the draft already selected. */
  async function rewriteAndRead(rowId: string, draft: string) {
    mockClient.answerAssist.mockResolvedValueOnce(assistOk(draft));
    await draftRow(rowId, { mode: 'rewrite', preset: 'shorten' });
    return stateOf(await send({ kind: 'answerSelectVersion', rowId, version: 0 })).rows;
  }

  it('does not append a new version, and sets a neutral (non-error) row notice', async () => {
    const rowId = await scanAndDraft(310);

    // "Shorten" comes back with only a trailing comma added — the measured
    // no-op shape (ADR-044 / the desktop's F3 twin).
    const rows = await rewriteAndRead(rowId, 'Led the migration and shipped the payment service,');

    // Mutation guard: without the unchanged-rewrite check this grows to 2 and
    // `notice` stays undefined — REVERT `settleRowFromAssist`'s rewrite branch
    // and this assertion is what catches it.
    expect(rows[0]?.versions).toHaveLength(1);
    expect(rows[0]?.notice).toMatch(/came back the same/);
    expect(rows[0]?.error).toBeUndefined();
  });

  it('still appends a genuinely different rewrite, and clears any stale notice', async () => {
    const rowId = await scanAndDraft(311);

    const rows = await rewriteAndRead(rowId, 'Led migration; shipped payments.');

    expect(rows[0]?.versions).toHaveLength(2);
    expect(rows[0]?.notice).toBeUndefined();
  });
});

// The lost-update race between the terminal stream mirror
// (broadcastAssistProgress → mirrorAssistToState) and settling the row
// (settleRowFromAssist), both of which read-modify-write the SAME tab's
// storage.session record with no serialization before answer-state.ts's per-tab
// write queue (pr-reviewer CRITICAL 2).
describe('the terminal stream-mirror write never clobbers the settled row (CRITICAL 2)', () => {
  it('a two-chunk draft ends with BOTH stream.done:true AND the drafted version appended, and the settled row renders with its controls enabled', async () => {
    const tabId = 900;
    const [rowId] = (await scanRows(tabId, 'Why this role?')) as [string];

    // Gate `storage.session.get` deterministically on the TERMINAL stream
    // mirror's own read — recognized by its CONTENT, not by which call number it
    // happens to be: the stored record's `stream.text` already equals the full
    // accumulated draft, but `stream.done` is still `false` (the terminal write
    // that flips it to `true` hasn't landed yet). Any OTHER read fails this
    // check, so an unrelated read added anywhere in the flow can never shift
    // which one gets gated. Capturing the stored value at CALL TIME but
    // resolving the returned promise only once this test releases `gate` is what
    // makes the race deterministic: under the OLD unserialized
    // `updateAnswerState`, settle's read+mutate+write (ungated) runs to
    // completion while this call is still pending, and this call's STALE
    // snapshot then clobbers settle's write the moment it is released.
    const FULL_DRAFT = 'Because I like solving real problems.';
    const sessionGetMock = vi.mocked(browser.storage.session.get);
    const realGet = sessionGetMock.getMockImplementation();
    if (!realGet) throw new Error('expected the default storage.session.get mock');
    let gated = false;
    let releaseGate: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseGate = resolve;
    });
    sessionGetMock.mockImplementation(((key: string) => {
      const read = realGet(key) as Promise<Record<string, unknown>>;
      return read.then((value) => {
        const state = value[key] as { stream?: { done?: boolean; text?: string } } | undefined;
        if (!gated && state?.stream?.done === false && state.stream.text === FULL_DRAFT) {
          gated = true;
          return gate.then(() => value);
        }
        return value;
      });
    }) as typeof realGet);

    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('Because I ');
        onChunk?.('like solving real problems.');
        return assistOk(FULL_DRAFT);
      }
    );

    const assistDone = draftRow(rowId);

    // Give settle's own (ungated) read+mutate+write a chance to run to
    // completion (under the OLD code) while the terminal mirror's read is still
    // stuck on `gate`, then release it and let everything settle.
    await flush();
    releaseGate?.();
    await assistDone;
    await flush();
    sessionGetMock.mockImplementation(realGet);

    const settled = await readAnswerState(tabId);
    if (!settled) throw new Error('expected a settled answer state');
    // Mutation guard: revert `updateAnswerState`'s per-tab queue (call
    // `readAnswerState`/`writeAnswerState` directly again) and EITHER of these
    // goes red, depending on which write lands last.
    expect(settled.stream?.done).toBe(true);
    expect(settled.rows.find((r) => r.id === rowId)?.versions).toHaveLength(1);

    // Close the loop into the UI (the same shared component both surfaces
    // mount): a lost `stream.done` combined with Finding 5's streaming gate
    // (`streaming = state.stream?.rowId === row.id && !state.stream.done`)
    // permanently disables every control on this row, on a FRESH mount, with no
    // escape on a single-question form.
    const { mountAnswerTools } = await import('../answer-tools/answer-tools');
    const host = document.createElement('div');
    document.body.append(host);
    const view = mountAnswerTools(host, { send, copy: vi.fn(async () => true) });
    view.render(settled);
    host.querySelector<HTMLButtonElement>('.arow__head')?.click();
    const regenerate = [...host.querySelectorAll<HTMLButtonElement>('.btn')].find(
      (b) => b.textContent === 'Regenerate'
    );
    expect(regenerate).not.toBeUndefined();
    expect(regenerate?.disabled).toBe(false);
  });
});

describe('a late activeTabId() must not re-arm a superseded run (regression)', () => {
  it('a run whose activeTabId() resolves AFTER a newer run already reset the buffer neither overwrites it nor fires its own billable request', async () => {
    const tabId = 851;
    const [rowA, rowB] = (await scanRows(tabId, 'Why this role?', 'What motivates you?')) as [
      string,
      string,
    ];
    const tabInfo = [{ id: tabId, url: `https://jobs.example.com/posting/${tabId}` } as never];

    // A request naming a `rowId` resolves through `runAnswerRowAssist` first,
    // which makes its OWN `activeTabId()` call before ever reaching
    // `runAnswerAssist` — so run A's three `tabs.query` calls, in order, are
    // `runAnswerRowAssist`'s `activeTabId()`, `runAnswerAssist`'s
    // `activeTabUrl()`, then `runAnswerAssist`'s OWN `activeTabId()` (the
    // bug-relevant one). Gate that THIRD call; run B starts and finishes
    // entirely afterward, landing on counts 4-6, never gated.
    let queryCalls = 0;
    let releaseGate: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseGate = resolve;
    });
    tabsQueryMock.mockImplementation(() => {
      queryCalls += 1;
      return queryCalls === 3 ? gate.then(() => tabInfo) : Promise.resolve(tabInfo);
    });

    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: (d: string) => void) => {
      onChunk?.('drafted');
      return assistOk('drafted', 'x');
    });

    // Run A: gets past the gen check with `assistTabId` still unresolved.
    const runA = draftRow(rowA);
    await flush();

    // Run B starts (bumps `assistGeneration`) and runs to completion while run A
    // is still suspended on the gate above.
    const runB = await draftRow(rowB, { question: 'What motivates you?' });
    expect(runB.ok).toBe(true);

    // Release run A's gate. Under the bug, A would resume believing it is still
    // current, clobber the buffer B just wrote, and fire its OWN billable
    // request — the exact thing the generation guard exists to stop.
    releaseGate?.();
    expect(await runA).toEqual({ ok: false, error: 'Superseded by a newer request.' });
    expect(mockClient.answerAssist).toHaveBeenCalledTimes(1);

    const settled = await readAnswerState(tabId);
    expect(settled?.stream?.rowId).toBe(rowB);
    expect(settled?.rows.find((r) => r.id === rowB)?.versions).toHaveLength(1);
    expect(settled?.rows.find((r) => r.id === rowA)?.versions).toHaveLength(0);

    tabsQueryMock.mockResolvedValue(tabInfo);
  });
});
