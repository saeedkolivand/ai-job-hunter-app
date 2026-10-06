/**
 * The Answer-tools section — ONE component, mounted by BOTH surfaces.
 *
 * ADR-044 decision 1 keeps the tools in the popup AND adds a side panel, as two
 * views of one state. Two views of one state only stays true if there is one
 * renderer: a forked copy would drift on its first bug fix, and the drift would
 * be invisible because each surface is only ever looked at on its own. So the
 * popup and the panel both mount THIS module against the same
 * `storage.session` record, and the only thing that differs between them is
 * the width they are laid out in (see `popup.css`'s `.arow` block, which the
 * panel document loads unchanged).
 *
 * What lives here is the row model's RENDERING and the ephemeral view state
 * that goes with it (which row is expanded, what is typed in its instruction
 * box). Everything a second surface has to agree about — rows, versions, the
 * selected version, the in-flight stream — lives in the shared state and is
 * only ever changed by asking the background.
 *
 * Note on primitives: this is the extension app, not the desktop renderer.
 * There is no React, no `@ajh/ui` and no i18n bundle here — the popup builds
 * plain DOM and ships English strings, and this module follows that same
 * convention rather than importing a renderer-only design system into an MV3
 * bundle.
 */

import {
  type AnswerRow,
  type AnswerState,
  counterText,
  isOverLimit,
  selectedText,
} from '../lib/answer-state';
import { button, el } from '../lib/dom';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import {
  acceptSentence,
  fitLimitChip,
  gatedOffNotice,
  groundedOnLine,
  iterationHint,
  LENGTH_CHIPS,
  PAGE_CHANGED_LINE,
  statusBadge,
  summaryLine,
  TONE_CHIPS,
} from './decisions';
import {
  renderActions,
  renderChipRow,
  renderVersionTabs,
  type RowControlsCtx,
} from './row-controls';

export * from './decisions';

// ── the view ────────────────────────────────────────────────────────────────

/**
 * Copy `text` to the clipboard; returns whether it succeeded. Extension pages
 * may call `navigator.clipboard.writeText` on a user gesture without any
 * extra permission — `clipboardRead` is on the manifest denylist and stays
 * there; WRITING needs nothing. Lives here rather than in each surface so the
 * popup and the panel cannot diverge on the one action that always works.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** What the host surface has to provide. Kept as an injected dependency so the
 *  component can be driven in a test without a background worker. */
export interface AnswerToolsDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  copy: (text: string) => Promise<boolean>;
}

/** The mounted component's handle. */
export interface AnswerToolsView {
  render: (state: AnswerState | null) => void;
}

/** The fixed `data-focus-key` for the always-visible "Add question" input —
 *  there is only one, so a constant is enough (a per-row key needs the row's
 *  own id; see `renderRowBody`'s instruction input). */
const ADD_QUESTION_FOCUS_KEY = 'add-question';

/**
 * What {@link captureFocus} saves about the one focused, `data-focus-key`-
 * tagged element inside `host`, so {@link restoreFocus} can put both the
 * caret and the focus back after a full rebuild.
 */
interface SavedFocus {
  key: string;
  start: number | null;
  end: number | null;
}

/**
 * Mount the Answer-tools section into `host`.
 *
 * The returned `render` is idempotent: it rebuilds the section from the state
 * it is given, so both the `storage.onChanged` push and a direct response can
 * drive it without either having to know what the other did.
 */
export function mountAnswerTools(host: HTMLElement, deps: AnswerToolsDeps): AnswerToolsView {
  /** The one expanded row (an accordion — the popup is 360 px wide and more
   *  than one open composer makes it unreadable). View-local: it is a way of
   *  looking at the state, not part of it. */
  let expandedRowId: string | null = null;
  /** Per-row free-instruction text, view-local for the same reason. */
  const instructions = new Map<string, string>();
  /** The "add a question" free-text input's value. Same rationale as
   *  `instructions`, but there is only one such input, so a single variable
   *  is enough — without it a fresh empty node was created on every render
   *  and typed text was wiped on every stream tick (Finding 1). */
  let addQuestionText = '';
  /** The last state rendered, so a local interaction can re-render without
   *  waiting for the storage round trip. */
  let current: AnswerState | null = null;
  /** Set while a request this view issued is in flight, so a double click
   *  cannot start two billable streams from one surface. */
  let busy = false;

  /**
   * `render()` does an unconditional `host.replaceChildren()` on every call —
   * including every streamed-token tick anywhere in the tab (Findings 1/2).
   * Capturing which `data-focus-key`-tagged element has focus (and its caret)
   * before the rebuild, then restoring it after, closes that generically for
   * any input this section renders, current or future, without switching the
   * render loop to incremental DOM patching.
   */
  function captureFocus(): SavedFocus | null {
    const active = document.activeElement;
    if (!(active instanceof HTMLElement) || !host.contains(active)) return null;
    const key = active.getAttribute('data-focus-key');
    if (!key) return null;
    const hasSelection =
      active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement;
    return {
      key,
      start: hasSelection ? active.selectionStart : null,
      end: hasSelection ? active.selectionEnd : null,
    };
  }

  function restoreFocus(saved: SavedFocus | null): void {
    if (!saved) return;
    for (const candidate of host.querySelectorAll<HTMLElement>('[data-focus-key]')) {
      if (candidate.getAttribute('data-focus-key') !== saved.key) continue;
      candidate.focus();
      if (
        (candidate instanceof HTMLInputElement || candidate instanceof HTMLTextAreaElement) &&
        saved.start !== null &&
        saved.end !== null
      ) {
        try {
          candidate.setSelectionRange(saved.start, saved.end);
        } catch {
          // Some input types (e.g. a future `type=number`) refuse a selection
          // range — losing the caret position is fine, losing focus is not.
        }
      }
      return;
    }
  }

  const rerender = (): void => render(current);

  const run = async (req: PopupRequest, onResult?: (res: PopupResponse) => void): Promise<void> => {
    if (busy) return;
    busy = true;
    rerender();
    try {
      const res = await deps.send(req);
      if (onResult) onResult(res);
      if (res.ok && res.kind === 'answerState') current = res.state;
    } catch (err) {
      setNotice(err instanceof Error ? err.message : String(err), 'err');
    } finally {
      busy = false;
      rerender();
    }
  };

  let noticeText = '';
  let noticeTone: 'ok' | 'err' = 'ok';
  const setNotice = (text: string, tone: 'ok' | 'err'): void => {
    noticeText = text;
    noticeTone = tone;
  };

  // ── row rendering ─────────────────────────────────────────────────────────

  const controls: RowControlsCtx = {
    copy: deps.copy,
    instructions,
    isBusy: () => busy,
    run,
    setNotice,
    rerender,
  };

  function renderRowBody(row: AnswerRow, state: AnswerState): HTMLElement {
    const body = el('div', 'arow__body');
    const streaming = state.stream?.rowId === row.id && !state.stream.done;
    const text = streaming ? (state.stream?.text ?? '') : selectedText(row);

    if (row.savedAnswer) {
      const saved = el('div', 'arow__saved');
      saved.append(el('p', 'arow__saved-title', 'You answered this before'));
      saved.append(el('p', 'arow__saved-body', row.savedAnswer));
      if (row.savedSource)
        saved.append(el('p', 'arow__saved-src', `from your ${row.savedSource} application`));
      const copySaved = button('btn btn--small btn--quiet', 'Copy saved answer');
      copySaved.addEventListener('click', () => {
        void deps.copy(row.savedAnswer ?? '').then((ok) => {
          setNotice(ok ? 'Copied.' : 'Could not copy.', ok ? 'ok' : 'err');
          rerender();
        });
      });
      saved.append(copySaved);
      body.append(saved);
    }

    const counter = counterText(row, text);
    if (counter) {
      const line = el('p', 'arow__counter', counter);
      if (isOverLimit(row, text)) line.classList.add('arow__counter--over');
      body.append(line);
    }

    if (!state.pageChanged) {
      body.append(renderChipRow(controls, row, 'Tone', TONE_CHIPS, streaming));
      const fit = fitLimitChip(row, text);
      body.append(
        renderChipRow(
          controls,
          row,
          'Length',
          fit ? [...LENGTH_CHIPS, fit] : LENGTH_CHIPS,
          streaming
        )
      );

      const instruction = el('input', 'arow__instruction');
      instruction.type = 'text';
      instruction.placeholder = 'Describe a change, or leave this empty…';
      instruction.setAttribute('aria-label', `Instruction for “${row.question}”`);
      instruction.setAttribute('data-focus-key', `instruction:${row.id}`);
      instruction.value = instructions.get(row.id) ?? '';
      instruction.addEventListener('input', () => instructions.set(row.id, instruction.value));
      body.append(instruction);
    }

    if (row.versions.length > 0) body.append(renderVersionTabs(controls, row));

    if (streaming) {
      const live = el('p', 'arow__text arow__text--live', text);
      live.setAttribute('role', 'status');
      body.append(live);
      const stopHint = el(
        'p',
        'arow__hint',
        'Drafting… this stays on screen if you close the popup.'
      );
      body.append(stopHint);
    } else if (text) {
      body.append(el('p', 'arow__text', text));
    }

    const grounded = groundedOnLine(row);
    if (grounded) body.append(el('p', 'arow__grounded', grounded));
    body.append(el('p', 'arow__hint', iterationHint(row)));

    const gated = gatedOffNotice(row.error);
    if (gated) {
      body.append(el('p', 'msg msg--err', gated));
    } else if (row.error) {
      body.append(el('p', 'msg msg--err', row.error));
    } else if (row.notice) {
      // Neutral, never `msg--err` — a no-op rewrite is not a failure.
      body.append(el('p', 'msg msg--muted', row.notice));
    }

    if (state.pageChanged) {
      body.append(el('p', 'msg msg--muted', PAGE_CHANGED_LINE));
    } else {
      const sentence = acceptSentence(row, state.pageChanged);
      if (sentence) body.append(el('p', 'arow__accept-note', sentence));
      body.append(renderActions(controls, row, text, state.pageChanged, streaming));
    }

    return body;
  }

  function renderRow(row: AnswerRow, state: AnswerState): HTMLElement {
    const wrap = el('div', 'arow');
    if (expandedRowId === row.id) wrap.classList.add('arow--open');

    const head = button('arow__head', '');
    head.setAttribute('data-focus-key', `head:${row.id}`);
    head.setAttribute('aria-expanded', String(expandedRowId === row.id));
    head.append(el('span', 'arow__q', row.question));
    head.append(el('span', `arow__badge arow__badge--${row.status}`, statusBadge(row)));
    head.addEventListener('click', () => {
      expandedRowId = expandedRowId === row.id ? null : row.id;
      rerender();
    });
    wrap.append(head);

    if (expandedRowId === row.id) wrap.append(renderRowBody(row, state));
    return wrap;
  }

  // ── section rendering ─────────────────────────────────────────────────────

  function render(state: AnswerState | null): void {
    current = state;
    const savedFocus = captureFocus();
    host.replaceChildren();

    const head = el('div', 'atools__head');
    head.append(el('p', 'atools__summary', summaryLine(state)));
    const rescan = button('btn btn--small btn--quiet', 'Rescan');
    rescan.setAttribute('data-focus-key', 'rescan');
    // Disabled once the page has changed (same signal every per-row write
    // control already gates on) — the line right below already tells the
    // user to use the toolbar icon instead (Finding 3).
    rescan.disabled = busy || Boolean(state?.pageChanged);
    rescan.title = 'Scan this page again — for a form that shows its questions a step at a time';
    rescan.addEventListener('click', () => {
      void run({ kind: 'answerScan' }, (res) => {
        if (!res.ok) setNotice(res.error, 'err');
      });
    });
    head.append(rescan);
    host.append(head);

    if (state?.pageChanged) host.append(el('p', 'msg msg--muted', PAGE_CHANGED_LINE));

    if (!state || state.rows.length === 0) {
      host.append(
        el(
          'p',
          'empty__body',
          'No questions found on this page yet. Open the application form, then rescan.'
        )
      );
    } else {
      const list = el('div', 'arows');
      for (const row of state.rows) list.append(renderRow(row, state));
      host.append(list);
    }

    // The free-text entry for a question the scan missed. Always available —
    // it needs no page access at all, so it keeps working after a navigation.
    // Backed by `addQuestionText` (Finding 1): without it this was the ONE
    // input on the section with no backing store at all, so a fresh empty
    // node was created — and typed text wiped — on every render.
    const addWrap = el('div', 'atools__add');
    const addInput = el('input', 'arow__instruction');
    addInput.type = 'text';
    addInput.placeholder = 'A question the scan missed…';
    addInput.setAttribute('aria-label', 'Add a question the scan missed');
    addInput.setAttribute('data-focus-key', ADD_QUESTION_FOCUS_KEY);
    addInput.value = addQuestionText;
    addInput.addEventListener('input', () => {
      addQuestionText = addInput.value;
    });
    const add = button('btn btn--small btn--quiet', 'Add question');
    add.setAttribute('data-focus-key', 'add-question-submit');
    const submitAdd = (): void => {
      const question = addInput.value.trim();
      if (!question) return;
      addQuestionText = '';
      addInput.value = '';
      void run({ kind: 'answerAddRow', question });
    };
    add.addEventListener('click', submitAdd);
    addInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') submitAdd();
    });
    addWrap.append(addInput, add);
    host.append(addWrap);

    if (noticeText) {
      const notice = el('p', `msg msg--${noticeTone === 'ok' ? 'ok' : 'err'}`, noticeText);
      notice.setAttribute('role', 'status');
      host.append(notice);
    }

    restoreFocus(savedFocus);
  }

  return { render };
}
