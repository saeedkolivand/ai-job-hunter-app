/**
 * The per-row controls of the Answer-tools section — rewrite chips, version
 * tabs and the Copy / Accept / Restore / Regenerate action row. Builders only:
 * the view state (`busy`, the instruction text, the notice) stays owned by
 * `answer-tools.ts`, which hands it in through {@link RowControlsCtx}.
 */

import { type AnswerRow, canAccept } from '../lib/answer-state';
import { button, el } from '../lib/dom';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import type { RewriteChip } from './decisions';

export interface RowControlsCtx {
  copy: (text: string) => Promise<boolean>;
  /** Per-row free-instruction text typed into the row's input. */
  instructions: ReadonlyMap<string, string>;
  /** Whether a request this view issued is in flight (read at render time). */
  isBusy: () => boolean;
  run: (req: PopupRequest, onResult?: (res: PopupResponse) => void) => Promise<void>;
  setNotice: (text: string, tone: 'ok' | 'err') => void;
  rerender: () => void;
}

export function renderChipRow(
  ctx: RowControlsCtx,
  row: AnswerRow,
  label: string,
  chips: readonly RewriteChip[],
  streaming: boolean
): HTMLElement {
  const wrap = el('div', 'chips');
  wrap.append(el('span', 'chips__label', label));
  for (const chip of chips) {
    const b = button('chip', chip.label);
    b.setAttribute('data-focus-key', `chip:${row.id}:${chip.label}`);
    if (!chip.preset && !chip.instruction) {
      // "As is" — the explicit neutral. It is a real control so the row does
      // not read as a required choice, and it deliberately does nothing.
      b.classList.add('chip--neutral');
      b.title = 'Leave this as it is';
      b.addEventListener('click', () => {
        ctx.setNotice('Left as it is.', 'ok');
        ctx.rerender();
      });
    } else {
      // Gated on the SHARED stream too, not just this view's own `busy`: a
      // stream started by another surface (or by this same view before a
      // remount) leaves `busy` false here while the row is still in flight
      // (Finding 5).
      b.disabled = ctx.isBusy() || streaming;
      b.addEventListener('click', () => {
        // Issue 1231 (Half A): a typed free instruction must never be
        // discarded by a chip. An instruction chip COMBINES both into one
        // instruction (chip directive first, then the typed text); a preset
        // chip sends its preset PLUS the typed text as `instruction` — the
        // server's resolver merges the pair, whereas a combined client-side
        // instruction string would reach the same model as unbounded
        // free-form text and lose the preset's fixed semantics.
        const typed = ctx.instructions.get(row.id)?.trim() ?? '';
        void ctx.run({
          kind: 'answerAssist',
          question: row.question,
          searchWeb: false,
          mode: 'rewrite',
          rowId: row.id,
          ...(chip.preset
            ? { preset: chip.preset, ...(typed ? { instruction: typed } : {}) }
            : chip.instruction
              ? { instruction: [chip.instruction, typed].filter(Boolean).join(' ') }
              : {}),
        });
      });
    }
    wrap.append(b);
  }
  return wrap;
}

export function renderVersionTabs(ctx: RowControlsCtx, row: AnswerRow): HTMLElement {
  const wrap = el('div', 'vtabs');
  const tabs = [
    { label: 'Original', index: -1, title: '' },
    ...row.versions.map((version, index) => ({
      label: version.label,
      index,
      title:
        version.kind === 'draft' ? 'A fresh grounded draft' : 'A reshape of the previous version',
    })),
  ];
  for (const { label, index, title } of tabs) {
    const tab = button('vtab', label);
    tab.setAttribute('aria-pressed', String(row.selected === index));
    if (row.selected === index) tab.classList.add('vtab--on');
    if (title) tab.title = title;
    tab.addEventListener('click', () => {
      void ctx.run({ kind: 'answerSelectVersion', rowId: row.id, version: index });
    });
    wrap.append(tab);
  }
  return wrap;
}

export function renderActions(
  ctx: RowControlsCtx,
  row: AnswerRow,
  text: string,
  pageChanged: boolean,
  streaming: boolean
): HTMLElement {
  const wrap = el('div', 'arow__actions');
  const locked = ctx.isBusy() || streaming;

  // Copy is the PRIMARY action: this is a copy-first tool, and it is the one
  // action that always works — no page access, no grant, no field.
  const copy = button('btn btn--small btn--primary', 'Copy');
  copy.disabled = text.trim().length === 0;
  copy.addEventListener('click', () => {
    void ctx.copy(text).then((ok) => {
      ctx.setNotice(
        ok ? 'Copied.' : 'Could not copy — select the text and copy it manually.',
        ok ? 'ok' : 'err'
      );
      ctx.rerender();
    });
  });
  wrap.append(copy);

  // Accept is the QUIET one, and it is ABSENT (not disabled) when there is
  // no field on the page to write into — a disabled button still claims the
  // capability exists.
  if (canAccept(row, pageChanged)) {
    const accept = button('btn btn--small btn--quiet', 'Accept into field');
    accept.disabled = locked;
    accept.addEventListener('click', () => {
      void ctx.run({ kind: 'answerAccept', rowId: row.id }, (res) => {
        if (!res.ok) return ctx.setNotice(res.error, 'err');
        if (res.kind !== 'answerAccept') return;
        ctx.setNotice(
          res.result.filled
            ? 'Written into the field.'
            : (res.result.error ?? 'Could not write into that field.'),
          res.result.filled ? 'ok' : 'err'
        );
      });
    });
    wrap.append(accept);

    if (row.field?.originalText) {
      const restore = button('btn btn--small btn--quiet', 'Restore original');
      restore.disabled = locked;
      restore.addEventListener('click', () => {
        void ctx.run({ kind: 'answerRestoreOriginal', rowId: row.id }, (res) => {
          if (!res.ok) ctx.setNotice(res.error, 'err');
        });
      });
      wrap.append(restore);
    }
  }

  const regenerate = button(
    'btn btn--small btn--quiet',
    row.versions.length ? 'Regenerate' : 'Draft this answer'
  );
  regenerate.disabled = locked;
  regenerate.addEventListener('click', () => {
    const typed = ctx.instructions.get(row.id);
    void ctx.run({
      kind: 'answerAssist',
      question: row.question,
      searchWeb: false,
      mode: 'draft',
      rowId: row.id,
      ...(typed?.trim() ? { instruction: typed } : {}),
    });
  });
  wrap.append(regenerate);
  return wrap;
}
