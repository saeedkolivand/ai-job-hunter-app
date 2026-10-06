/**
 * The mounted Answer-tools section (`mountAnswerTools`): what a rendered row
 * offers, what a chip sends, and what survives the full rebuild every `render()`
 * does. The pure copy and decisions are pinned in `decisions.test.ts`.
 */

import { describe, expect, it, vi } from 'vitest';

import { EXTENSION_AI_ASSIST_OFF_MESSAGE } from '@ajh/shared/extension-protocol';

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { mountAnswerTools } from './answer-tools';
import { row, stateOf } from './test-support';

describe('mountAnswerTools (the rendered row)', () => {
  /** One drafted row whose field is capped at 10 characters (the draft is longer). */
  const state = (over: Partial<AnswerState> = {}): AnswerState =>
    stateOf({
      origin: 'https://boards.example.com',
      rows: [
        row({
          status: 'drafted',
          selected: 0,
          versions: [{ label: 'v1', text: 'A drafted answer.', kind: 'draft' }],
          field: {
            kind: 'empty',
            index: 0,
            count: 1,
            currentText: '',
            originalText: '',
            maxChars: 10,
          },
        }),
      ],
      ...over,
    });

  const mount = (
    send = vi.fn(async (): Promise<PopupResponse> => ({
      ok: true,
      kind: 'answerState',
      state: null,
    }))
  ) => {
    const host = document.createElement('div');
    document.body.append(host);
    const view = mountAnswerTools(host, { send, copy: vi.fn(async () => true) });
    return { host, view, send };
  };

  /** Mount, render `over` and (by default) expand the first row. */
  const shown = (over: Partial<AnswerState> = {}, open = true) => {
    const mounted = mount();
    mounted.view.render(state(over));
    if (open) mounted.host.querySelector<HTMLButtonElement>('.arow__head')?.click();
    return mounted;
  };

  const chips = (host: HTMLElement) => [...host.querySelectorAll<HTMLButtonElement>('.chip')];
  const chip = (host: HTMLElement, label: string) =>
    chips(host).find((b) => b.textContent === label);
  const rescanOf = (host: HTMLElement) =>
    [...host.querySelectorAll<HTMLButtonElement>('.atools__head button')].find(
      (b) => b.textContent === 'Rescan'
    );
  const buttons = (host: HTMLElement) => [...host.querySelectorAll<HTMLButtonElement>('.btn')];

  it('renders the composer for the open row without touching the page', () => {
    const { host } = shown();

    expect(host.textContent).toContain('A drafted answer.');
    // The counter measures the text on screen against the field's own limit.
    expect(host.textContent).toContain('17 / 10 characters');
    // Copy is the primary action; Accept is the quiet one beside it.
    expect(host.querySelector('.btn--primary')?.textContent).toBe('Copy');
    expect(host.textContent).toContain('Accept into field');
    expect(host.textContent).toContain('Nothing else is touched');
    // Over the limit, so the fit-the-limit chip is offered — and only then.
    expect(host.textContent).toContain('Fit 10');
  });

  it('replaces every write control with one line after a navigation, keeping the rows', () => {
    const { host } = shown({ pageChanged: true });

    // The row and its draft are still there — decision 3 keeps them.
    expect(host.textContent).toContain('A drafted answer.');
    expect(host.textContent).toContain('Click the toolbar icon');
    // …and nothing that would read or write the page is offered.
    expect(host.textContent).not.toContain('Accept into field');
    expect(host.querySelector('.chip')).toBeNull();
  });

  it('offers no Accept for a question that is not on the page', () => {
    const { host } = shown({
      rows: [
        row({
          field: null,
          status: 'drafted',
          selected: 0,
          versions: [{ label: 'v1', text: 'Copy-only.', kind: 'draft' }],
        }),
      ],
    });

    expect(host.textContent).toContain('Copy-only.');
    expect(host.textContent).not.toContain('Accept into field');
    expect(host.textContent).not.toContain('Nothing else is touched');
  });

  it('shows the gated-off sentinel with where to turn it on, and what still works', () => {
    const { host } = shown({ rows: [row({ error: EXTENSION_AI_ASSIST_OFF_MESSAGE })] });

    expect(host.textContent).toContain(EXTENSION_AI_ASSIST_OFF_MESSAGE);
    expect(host.textContent).toContain('keep working while drafting is off');
  });

  it('sends a rewrite over the existing wire verb when a chip is pressed', () => {
    const { host, send } = shown();

    chip(host, 'Shorter')?.click();

    expect(send).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'answerAssist', mode: 'rewrite', preset: 'shorten' })
    );
  });

  it('does not send anything for the explicit "As is" neutral', () => {
    const { host, send } = shown();

    chips(host)
      .filter((b) => b.textContent === 'As is')
      .forEach((b) => b.click());

    expect(send).not.toHaveBeenCalled();
  });

  it('renders the "Left as it is." notice immediately on click — the ONLY feedback this no-op control gives (regression)', () => {
    const { host } = shown();

    chip(host, 'As is')?.click();

    expect(host.querySelector('.msg')?.textContent).toBe('Left as it is.');
  });

  // ── render() tears the whole section down on every call (a stream tick
  // anywhere in the tab, via storage.onChanged) — Findings 1/2 ────────────────

  it('keeps the "add a question" input\'s typed text AND focus/caret across a re-render', () => {
    const { host, view } = shown({}, false);

    const addInput = host.querySelector<HTMLInputElement>('[data-focus-key="add-question"]');
    if (!addInput) throw new Error('expected the add-question input');
    addInput.value = 'What is your salary expectation?';
    addInput.dispatchEvent(new Event('input'));
    addInput.focus();
    addInput.setSelectionRange(4, 8);

    // Simulates a `storage.onChanged` push for a stream progressing on ANY
    // row in the tab — `render()` rebuilds unconditionally on every call.
    view.render(state());

    const restored = host.querySelector<HTMLInputElement>('[data-focus-key="add-question"]');
    expect(restored).not.toBeNull();
    // Mutation guard: without the value backing, this is '' — without focus
    // restoration, `document.activeElement` is `document.body`.
    expect(restored?.value).toBe('What is your salary expectation?');
    expect(restored).toBe(document.activeElement);
    expect(restored?.selectionStart).toBe(4);
    expect(restored?.selectionEnd).toBe(8);
  });

  it("keeps a row's instruction input FOCUSED (with its caret) across a re-render", () => {
    const { host, view } = shown();

    const instruction = host.querySelector<HTMLInputElement>('.arow__instruction');
    if (!instruction) throw new Error('expected the row instruction input');
    instruction.value = 'Mention Berlin';
    instruction.dispatchEvent(new Event('input'));
    instruction.focus();
    instruction.setSelectionRange(2, 2);

    view.render(state());

    const restored = host.querySelector<HTMLInputElement>('.arow__instruction');
    // Mutation guard: without focus restoration this is `document.body`.
    expect(restored).toBe(document.activeElement);
    expect(restored?.selectionStart).toBe(2);
  });

  // Mutation guard for both: without a `data-focus-key` on the button, focus
  // lands on `document.body` after every render — several times a second while
  // anything streams.
  it.each([
    ['Rescan button', '[data-focus-key="rescan"]', false],
    ["row's head toggle", '.arow__head', true],
  ])('keeps the %s FOCUSED across a re-render', (_name, selector, open) => {
    const { host, view } = shown({}, open);

    const target = host.querySelector<HTMLButtonElement>(selector);
    if (!target) throw new Error(`expected ${selector}`);
    target.focus();

    view.render(state());

    const restored = host.querySelector<HTMLButtonElement>(selector);
    expect(restored).not.toBeNull();
    expect(restored).toBe(document.activeElement);
  });

  // ── Rescan vs `pageChanged` and a RESOLVED (not thrown) failure — Finding 3 ─

  it('disables Rescan once the page has changed, mirroring the per-row write controls', () => {
    const { host } = shown({ pageChanged: true }, false);

    expect(rescanOf(host)?.disabled).toBe(true);
  });

  it('surfaces a RESOLVED answerScan failure (not just a thrown one) as an error notice', async () => {
    const { host, view } = mount(
      vi.fn(async (req: PopupRequest): Promise<PopupResponse> =>
        req.kind === 'answerScan'
          ? { ok: false, error: 'Could not read the questions on this page.' }
          : { ok: true, kind: 'answerState', state: null }
      )
    );
    view.render(state({ rows: [] }));

    rescanOf(host)?.click();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(host.querySelector('.msg--err')?.textContent).toBe(
      'Could not read the questions on this page.'
    );
  });

  // ── The SHARED stream gates controls, not just this view's own `busy` —
  // Finding 5 (a stream another surface started, or this view reattaching
  // mid-stream, leaves `busy` false here) ─────────────────────────────────────

  const streamOn = (rowId: string): Partial<AnswerState> => ({
    stream: { rowId, text: 'partial answer', done: false, interrupted: false, kind: 'rewrite' },
  });

  it('disables chips/Accept/Regenerate while the shared stream targets this row, even with busy=false', () => {
    const { host } = shown(streamOn('r'));

    const nonNeutralChips = chips(host).filter((b) => !b.classList.contains('chip--neutral'));
    expect(nonNeutralChips.length).toBeGreaterThan(0);
    // Mutation guard: without gating on `streaming`, every one of these is
    // enabled here (`busy` starts `false` on a fresh mount).
    expect(nonNeutralChips.every((b) => b.disabled)).toBe(true);

    expect(buttons(host).find((b) => b.textContent === 'Accept into field')?.disabled).toBe(true);
    expect(buttons(host).find((b) => b.textContent === 'Regenerate')?.disabled).toBe(true);
  });

  it('does NOT disable controls for a stream on a DIFFERENT row', () => {
    const { host } = shown(streamOn('some-other-row'));

    expect(buttons(host).find((b) => b.textContent === 'Accept into field')?.disabled).toBe(false);
  });
});
