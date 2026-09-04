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

// The dedicated `extension-protocol` entrypoint, not the `@ajh/shared` barrel:
// this is a RUNTIME import, and the barrel drags zod and the whole IPC surface
// into an MV3 bundle that must stay reviewable and small (measured: 98.8 kB of
// chunk vs 4.2 kB). Same reason `lib/bridge.ts` imports from there. TYPE-only
// imports may keep using the barrel — they are erased at build.
import {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
  type ExtensionRewritePreset,
} from '@ajh/shared/extension-protocol';

import {
  type AnswerRow,
  type AnswerState,
  canAccept,
  counterText,
  isOverLimit,
  selectedText,
} from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

// ── pure copy + decisions (exported for unit tests) ──────────────────────────

/**
 * The two shared refusal sentinels, as a set. The desktop's wire-error
 * discipline is fixed sentinel TEXT rather than a machine-readable code (the
 * sentinel IS the code — `docs/knowledge/extension-domain.md`), and these are
 * the constants the desktop declares beside the handler, so matching them is
 * matching the source of truth rather than a copied string.
 */
const REFUSAL_SENTINELS: ReadonlySet<string> = new Set([
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
]);

/**
 * What a gated-off row should say, or `null` when this error is not one of the
 * two sentinels and must therefore be rendered verbatim like every other
 * opaque wire error.
 *
 * The sentinel text already names the setting to turn on, so this adds only
 * the part the desktop cannot know: which of THIS row's controls keep working
 * while drafting is off. Saying "AI is off" and leaving the row looking dead
 * is the failure mode being avoided.
 */
export function gatedOffNotice(error: string | undefined): string | null {
  if (error === undefined || !REFUSAL_SENTINELS.has(error)) return null;
  return `${error} Saved answers, the version history and the character counter keep working while drafting is off.`;
}

/** The badge text for a row's status. */
export function statusBadge(row: AnswerRow): string {
  switch (row.status) {
    case 'saved-available':
      return 'Saved answer';
    case 'filled':
      return 'Filled';
    case 'drafted':
      return `${row.versions[row.versions.length - 1]?.label ?? 'v1'} ready`;
    default:
      return row.field === null ? 'Not on page' : 'Empty';
  }
}

/**
 * The Accept sentence, naming the EXACT question it overwrites (ADR-044
 * decision 4's honesty requirement) — `null` when there is no Accept to
 * explain, so the sentence can never appear beside a disabled or absent
 * button and promise something that will not happen.
 */
export function acceptSentence(row: AnswerRow, pageChanged: boolean): string | null {
  if (!canAccept(row, pageChanged)) return null;
  return `Accept replaces the answer in “${row.question}” on the page. Nothing else is touched.`;
}

/**
 * The "grounded on" line for the version on screen, built from the wire's own
 * `sourced` flags plus the résumé (which draft mode always uses and therefore
 * never reports as a flag). `null` for a REWRITE and for the page's own text:
 * a rewrite is a pure text transform that never sees the résumé or the
 * posting, so claiming grounding for it would be the exact overstatement
 * decision 5 exists to stop.
 */
export function groundedOnLine(row: AnswerRow): string | null {
  const version = row.versions[row.selected];
  if (!version || version.kind !== 'draft') return null;
  const parts = ['your résumé'];
  if (version.sourced?.brief) parts.push('this posting');
  if (version.sourced?.salary) parts.push('the saved salary range');
  if (version.sourced?.web) parts.push('web search');
  return `Grounded on: ${parts.join(' · ')}`;
}

/**
 * The one honest line about what the chips do, versus Regenerate — a
 * function of the ROW rather than a flat constant, because the "and this
 * posting" clause is only true when the row's own selected draft actually
 * used one ({@link groundedOnLine}'s same `sourced.brief` signal). A row
 * drafted before a job was matched must not claim grounding Regenerate would
 * not actually have.
 */
export function iterationHint(row: AnswerRow): string {
  const version = row.versions[row.selected];
  const groundedOnPosting = version?.kind === 'draft' && version.sourced?.brief === true;
  return groundedOnPosting
    ? 'Chips reshape this text. Regenerate rethinks it from your résumé and this posting.'
    : 'Chips reshape this text. Regenerate rethinks it from your résumé.';
}

/** The line that replaces every write control after a navigation. */
export const PAGE_CHANGED_LINE =
  'This page changed. Click the toolbar icon to re-grant access and scan it — your drafts below are kept.';

/** Header summary: how many questions, and how many still need an answer. */
export function summaryLine(state: AnswerState | null): string {
  if (!state || state.rows.length === 0) return 'Nothing scanned yet';
  const total = state.rows.length;
  const answered = state.rows.filter((r) => r.status === 'filled' || r.status === 'drafted').length;
  const noun = total === 1 ? 'question' : 'questions';
  return `${total} ${noun} · ${total - answered} to go`;
}

/** One rewrite chip: a label plus what it sends over the existing wire verb. */
export interface RewriteChip {
  label: string;
  preset?: ExtensionRewritePreset;
  instruction?: string;
}

/**
 * TONE chips. Every one is a REWRITE of the latest version through the wire's
 * existing rewrite mode — no protocol change (decision 5). Two of them map to
 * a server-side preset; the rest carry a free instruction, because a preset
 * always beats free text server-side and there is no preset for "warmer".
 *
 * The leading "As is" is deliberate and does nothing: without an explicit
 * neutral the chip row reads as a required choice, and a user who likes the
 * tone has to guess that not pressing anything is allowed.
 */
export const TONE_CHIPS: readonly RewriteChip[] = [
  { label: 'As is' },
  {
    label: 'Warmer',
    instruction: 'Make this warmer and more personal, keeping every concrete fact.',
  },
  {
    label: 'Formal',
    instruction: 'Make this more formal and professional, keeping every concrete fact.',
  },
  {
    label: 'Simpler',
    instruction: 'Make this plainer and easier to read, keeping every concrete fact.',
  },
  { label: 'More impact', preset: 'impact' },
  { label: 'Fix grammar', preset: 'grammar' },
];

/** LENGTH chips. Same rewrite path as {@link TONE_CHIPS}, same explicit
 *  "As is". The fit-the-limit chip is added separately because it only exists
 *  when there IS a limit and the text is over it. */
export const LENGTH_CHIPS: readonly RewriteChip[] = [
  { label: 'As is' },
  { label: 'Shorter', preset: 'shorten' },
  { label: 'Longer', preset: 'expand' },
];

/**
 * The fit-the-limit chip for a row that is over its field's own `maxlength`,
 * or `null` when there is no limit or the text already fits. It carries the
 * MEASURED overshoot rather than asking the model to count: the count is
 * taken here, from the text on screen.
 */
export function fitLimitChip(row: AnswerRow, text: string): RewriteChip | null {
  const limit = row.field?.maxChars;
  if (limit === undefined || !isOverLimit(row, text)) return null;
  return {
    label: `Fit ${limit}`,
    instruction: `This is ${text.length} characters; the limit is ${limit}. Cut at least ${text.length - limit} characters, keeping every concrete fact.`,
  };
}

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

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

const button = (className: string, label: string): HTMLButtonElement => {
  const b = el('button', className, label);
  b.type = 'button';
  return b;
};

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

  function renderChipRow(
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
          setNotice('Left as it is.', 'ok');
          rerender();
        });
      } else {
        // Gated on the SHARED stream too, not just this view's own `busy`: a
        // stream started by another surface (or by this same view before a
        // remount) leaves `busy` false here while the row is still in flight
        // (Finding 5).
        b.disabled = busy || streaming;
        b.addEventListener('click', () => {
          void run({
            kind: 'answerAssist',
            question: row.question,
            searchWeb: false,
            mode: 'rewrite',
            rowId: row.id,
            ...(chip.preset ? { preset: chip.preset } : {}),
            ...(chip.instruction ? { instruction: chip.instruction } : {}),
          });
        });
      }
      wrap.append(b);
    }
    return wrap;
  }

  function renderVersionTabs(row: AnswerRow): HTMLElement {
    const wrap = el('div', 'vtabs');
    const original = button('vtab', 'Original');
    original.setAttribute('aria-pressed', String(row.selected === -1));
    if (row.selected === -1) original.classList.add('vtab--on');
    original.addEventListener('click', () => {
      void run({ kind: 'answerSelectVersion', rowId: row.id, version: -1 });
    });
    wrap.append(original);

    row.versions.forEach((version, i) => {
      const tab = button('vtab', version.label);
      tab.setAttribute('aria-pressed', String(row.selected === i));
      if (row.selected === i) tab.classList.add('vtab--on');
      tab.title =
        version.kind === 'draft' ? 'A fresh grounded draft' : 'A reshape of the previous version';
      tab.addEventListener('click', () => {
        void run({ kind: 'answerSelectVersion', rowId: row.id, version: i });
      });
      wrap.append(tab);
    });
    return wrap;
  }

  function renderActions(
    row: AnswerRow,
    text: string,
    pageChanged: boolean,
    streaming: boolean
  ): HTMLElement {
    const wrap = el('div', 'arow__actions');

    // Copy is the PRIMARY action: this is a copy-first tool, and it is the one
    // action that always works — no page access, no grant, no field.
    const copy = button('btn btn--small btn--primary', 'Copy');
    copy.disabled = text.trim().length === 0;
    copy.addEventListener('click', () => {
      void deps.copy(text).then((ok) => {
        setNotice(
          ok ? 'Copied.' : 'Could not copy — select the text and copy it manually.',
          ok ? 'ok' : 'err'
        );
        rerender();
      });
    });
    wrap.append(copy);

    // Accept is the QUIET one, and it is ABSENT (not disabled) when there is
    // no field on the page to write into — a disabled button still claims the
    // capability exists.
    if (canAccept(row, pageChanged)) {
      const accept = button('btn btn--small btn--quiet', 'Accept into field');
      accept.disabled = busy || streaming;
      accept.addEventListener('click', () => {
        void run({ kind: 'answerAccept', rowId: row.id }, (res) => {
          if (!res.ok) return setNotice(res.error, 'err');
          if (res.kind !== 'answerAccept') return;
          setNotice(
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
        restore.disabled = busy || streaming;
        restore.addEventListener('click', () => {
          void run({ kind: 'answerRestoreOriginal', rowId: row.id }, (res) => {
            if (!res.ok) setNotice(res.error, 'err');
          });
        });
        wrap.append(restore);
      }
    }

    const regenerate = button(
      'btn btn--small btn--quiet',
      row.versions.length ? 'Regenerate' : 'Draft this answer'
    );
    regenerate.disabled = busy || streaming;
    regenerate.addEventListener('click', () => {
      void run({
        kind: 'answerAssist',
        question: row.question,
        searchWeb: false,
        mode: 'draft',
        rowId: row.id,
        ...(instructions.get(row.id)?.trim() ? { instruction: instructions.get(row.id) } : {}),
      });
    });
    wrap.append(regenerate);
    return wrap;
  }

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
      body.append(renderChipRow(row, 'Tone', TONE_CHIPS, streaming));
      const fit = fitLimitChip(row, text);
      body.append(
        renderChipRow(row, 'Length', fit ? [...LENGTH_CHIPS, fit] : LENGTH_CHIPS, streaming)
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

    if (row.versions.length > 0) body.append(renderVersionTabs(row));

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
      body.append(renderActions(row, text, state.pageChanged, streaming));
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
