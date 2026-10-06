/**
 * "Help me answer…" — the first BILLABLE-AI verb on the bridge. Owns the single
 * streaming buffer (so a popup that closes mid-stream and reopens can reattach)
 * and folds a settled reply into its answer row.
 */

import type { ExtensionAnswerAssistRequest } from '@ajh/shared';
// Runtime import, so it comes from the dedicated entrypoint rather than the
// barrel — see the note at the top of `answer-tools/answer-tools.ts`.
import { EXTENSION_ANSWER_ASSIST_MAX_CHARS } from '@ajh/shared/extension-protocol';

import {
  type AnswerRow,
  appendVersion,
  isUnchangedRewrite,
  rewriteBaseText,
  updateAnswerState,
} from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { getClient, notPaired, pushToSurfaces } from './bridge-client';
import { activeTabId, activeTabUrl } from './page';

/** Append `delta` to `text`, clamped to the shared
 *  {@link EXTENSION_ANSWER_ASSIST_MAX_CHARS} cap (the Rust
 *  `answer_assist::DRAFT_CAP` mirrors it). The desktop already clamps each
 *  `assist.chunk`, so this exists only so the interrupted/error path can never
 *  render an unbounded draft even if that server-side guarantee were violated. */
function growAssistDraft(text: string, delta: string): string {
  const grown = text + delta;
  return grown.length > EXTENSION_ANSWER_ASSIST_MAX_CHARS
    ? grown.slice(0, EXTENSION_ANSWER_ASSIST_MAX_CHARS)
    : grown;
}

/**
 * The CURRENT (or last-finished) streaming `answer.assist` buffer — owned HERE,
 * not by the popup (see `PopupResponse`'s `answerAssistProgress` doc).
 * Single-slot: a new `runAnswerAssist` call always resets it. `interrupted` is
 * set only when the stream ended in failure AFTER some text had accumulated (a
 * clean upfront refusal is a normal error, not an interruption).
 *
 * "Button disabled while in flight" alone does NOT keep this single slot safe:
 * an MV3 popup is torn down on close, so one that closes mid-stream and
 * reopens shows a fresh, enabled button and can re-trigger `runAnswerAssist`
 * while the first run is still in flight. {@link assistGeneration} makes
 * overlap safe.
 */
let assistBuffer: {
  text: string;
  done: boolean;
  interrupted: boolean;
  /** Which answer ROW this stream belongs to (ADR-044 decision 1), or `''` when
   *  the caller has no row model. Rides on the buffer so a superseding run
   *  replaces the text and its owner atomically. */
  rowId: string;
  /** `draft` (grounded, Regenerate) vs `rewrite` (reshape of the previous version). */
  kind: 'draft' | 'rewrite';
  /** Present ONLY for a Prep tab on-demand draft — a caller with no row model
   *  tags its stream by `topic` instead (see `lib/answer-state.ts`). */
  topic: ExtensionAnswerAssistRequest['topic'] | null;
} = { text: '', done: true, interrupted: false, rowId: '', kind: 'draft', topic: null };

/** The tab whose shared answer state {@link assistBuffer} is mirrored into —
 *  captured when a run starts, because the active tab can change under a long
 *  stream and the mirror must reach the SAME `storage.session` record. */
let assistTabId: number | null = null;

/**
 * Single-flight generation counter for {@link assistBuffer}. `runAnswerAssist`
 * captures its own value on entry, superseding any prior run; a run whose
 * captured value no longer matches has been superseded by a newer overlapping
 * call and must skip every `assistBuffer` write (chunks and terminal alike).
 */
let assistGeneration = 0;

/** The buffer as a popup-facing message — for both the live push and the
 *  `answerAssistProgress` reattach query. */
export function assistProgress(): PopupResponse {
  return {
    ok: true,
    kind: 'answerAssistProgress',
    text: assistBuffer.text,
    done: assistBuffer.done,
    interrupted: assistBuffer.interrupted,
    rowId: assistBuffer.rowId,
  };
}

/** Mirror the buffer into the shared answer state, so a closed (or never-open)
 *  popup and a panel on another surface both see the same stream. Best-effort:
 *  a failed session write must never fail the user's click. */
async function mirrorAssistToState(): Promise<void> {
  const tabId = assistTabId;
  // A caller with a row model tags by `rowId`; a Prep tab draft (no row model)
  // tags by `topic` — either is enough, neither alone is never mirrored.
  if (tabId === null || (!assistBuffer.rowId && !assistBuffer.topic)) return;
  const snapshot = { ...assistBuffer };
  await updateAnswerState(tabId, (state) => ({
    ...state,
    stream: {
      rowId: snapshot.rowId,
      text: snapshot.text,
      done: snapshot.done,
      interrupted: snapshot.interrupted,
      kind: snapshot.kind,
      ...(snapshot.topic ? { topic: snapshot.topic } : {}),
    },
  }));
}

function broadcastAssistProgress(): void {
  void mirrorAssistToState();
  void pushToSurfaces(assistProgress);
}

/**
 * Cancel whatever `answer.assist` stream is pending (the Prep tab's Cancel).
 * Always `ok:true` — a no-op when nothing was pending is not an error.
 *
 * Two things make the cancel stick to a run instead of racing past it:
 *  1. The generation bump. `runAnswerAssist` awaits `getToken` / `activeTabUrl`
 *     / `activeTabId` BEFORE it sends the (billable) request. A cancel landing
 *     during those awaits used to call `cancelCurrent()` on an EMPTY pending
 *     request — a no-op — and the run then sent anyway. Raising
 *     `assistGeneration` makes that run fail its own post-await guard before any
 *     byte is sent (a NEW run re-bumps the counter on entry).
 *  2. The terminal buffer mark. Without it a mid-stream cancel left
 *     `done:false` until the cancelled request settled. `done:true` +
 *     `interrupted` (true only when text had accumulated) settles every surface
 *     immediately; the `!done` guard keeps a cancel after a finish a pure
 *     generation bump.
 */
export function runAssistCancel(): PopupResponse {
  assistGeneration += 1;
  if (!assistBuffer.done) {
    assistBuffer = { ...assistBuffer, done: true, interrupted: assistBuffer.text.length > 0 };
    broadcastAssistProgress();
  }
  getClient().cancelCurrent();
  return { ok: true, kind: 'assistCancel' };
}

/** Bound a page-derived character limit to the shared wire ceiling. Rejects
 *  anything that is not a positive integer — a page that writes
 *  `maxlength="abc"` or a negative value must contribute nothing. */
function clampMaxChars(value: number | undefined): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  const floored = Math.floor(value);
  if (floored <= 0) return undefined;
  return Math.min(floored, EXTENSION_ANSWER_ASSIST_MAX_CHARS);
}

/** The neutral notice for a chip rewrite that came back unchanged (measured
 *  live, same defect class as the desktop's F3 — see `isUnchangedRewrite`). */
const UNCHANGED_REWRITE_NOTICE =
  'That came back the same — try Regenerate for a fresh draft, or a different instruction.';

/**
 * Fold a settled `answer.assist` reply into its row: a success appends the new
 * version (and selects it), a refusal records the error text verbatim so the
 * view can match the shared sentinels. Never throws.
 *
 * A REWRITE (never a draft — Regenerate is expected to differ, a chip is not)
 * whose result is unchanged from the version it reshaped is not appended at
 * all: that would present a no-op as if it worked, with Accept enabled on text
 * identical to what is on screen. It becomes a neutral per-row
 * {@link AnswerRow.notice} instead.
 */
async function settleRowFromAssist(
  rowId: string,
  kind: 'draft' | 'rewrite',
  result:
    | { ok: true; draft: string; sourced: Record<string, boolean | undefined> }
    | { ok: false; error: string }
): Promise<void> {
  if (assistTabId === null) return;
  const patchRow = (rows: AnswerRow[], patch: (row: AnswerRow) => AnswerRow): AnswerRow[] =>
    rows.map((row) => (row.id === rowId ? patch(row) : row));
  await updateAnswerState(assistTabId, (state) => {
    if (!result.ok) {
      return {
        ...state,
        rows: patchRow(state.rows, (row) => {
          const next: AnswerRow = { ...row, error: result.error };
          delete next.notice;
          return next;
        }),
      };
    }
    if (kind === 'rewrite') {
      const row = state.rows.find((r) => r.id === rowId);
      if (row && isUnchangedRewrite(rewriteBaseText(row), result.draft)) {
        return {
          ...state,
          rows: patchRow(state.rows, (r) => {
            const next: AnswerRow = { ...r, notice: UNCHANGED_REWRITE_NOTICE };
            delete next.error;
            return next;
          }),
        };
      }
    }
    // A rewrite is grounded on nothing, so it carries no flags at all rather
    // than three falses that would render an empty "grounded on" line.
    const sourced =
      kind === 'draft'
        ? {
            web: result.sourced.web === true,
            brief: result.sourced.brief === true,
            salary: result.sourced.salary === true,
          }
        : undefined;
    return { ...state, rows: appendVersion(state.rows, rowId, result.draft, kind, sourced) };
  });
}

/** The popup request minus its discriminant — also built directly by row-scoped callers. */
type AssistRun = Omit<Extract<PopupRequest, { kind: 'answerAssist' }>, 'kind'>;

/**
 * Draft (`mode` omitted) or rewrite (`mode: 'rewrite'`) — both ride the SAME
 * opt-in, streaming path and single-flight buffer; only the payload fields
 * forwarded to the desktop differ. A deliberate click: failures propagate to
 * the dispatcher's outer catch. Sends the active tab's url too (when readable)
 * so the desktop can ground a draft on a matched Application; a url-read
 * failure degrades to generic grounding rather than blocking the request.
 *
 * The desktop STREAMS the answer: this resets `assistBuffer` and accumulates
 * each `assist.chunk` delta into it (broadcasting a push per chunk), so a popup
 * that reopens mid-stream can reattach via `{kind:'answerAssistProgress'}`. On
 * any settle the buffer is marked `done`; `interrupted` only when text had
 * already accumulated before the failure.
 *
 * Single-flight via {@link assistGeneration}. The `gen` captured on entry
 * supersedes any prior run, and two guards cover the two windows a superseded
 * run could clobber the buffer in:
 *   - BEFORE the reset (its own `getToken`/`activeTabUrl` awaits can still be
 *     pending after a newer call already reset AND finished the buffer) — the
 *     early bail means it never resets that buffer and never issues its own
 *     (billable) streaming request.
 *   - DURING the stream — each chunk AND the terminal write on both the success
 *     and the error path re-check `gen` and are a no-op when it doesn't match
 *     (the result/rethrow still happen so this run's own caller settles).
 */
export async function runAnswerAssist(req: AssistRun): Promise<PopupResponse> {
  const { question, searchWeb, mode, existingAnswer, preset, instruction, rowId, topic } = req;
  const gen = ++assistGeneration;
  const streamKind: 'draft' | 'rewrite' = mode === 'rewrite' ? 'rewrite' : 'draft';

  if (!(await getToken())) return notPaired();

  const url = await activeTabUrl(req.windowId).catch(() => undefined);
  const tabIdForRun = await activeTabId(req.windowId).catch(() => null);

  // A newer overlapping call already reset (and may have finished) the buffer
  // while the awaits above were pending — this run must not reset it back to
  // `done:false`, must not broadcast, and must not make its own billable
  // request. No await separates this check from the reset below.
  if (gen !== assistGeneration) {
    return { ok: false, error: 'Superseded by a newer request.' };
  }

  assistTabId = tabIdForRun;
  assistBuffer = {
    text: '',
    done: false,
    interrupted: false,
    rowId: rowId ?? '',
    kind: streamKind,
    topic: topic ?? null,
  };
  broadcastAssistProgress();

  const payload: ExtensionAnswerAssistRequest = { question, searchWeb };
  if (url) payload.url = url;
  if (mode) payload.mode = mode;
  if (existingAnswer !== undefined) payload.existingAnswer = existingAnswer;
  if (preset) payload.preset = preset;
  if (instruction) payload.instruction = instruction;
  if (topic) payload.topic = topic;
  // DRAFT MODE ONLY (ADR-044 decision 6): the wire ignores the limit in rewrite
  // mode, so sending it there would be a claim the desktop does not honour. The
  // value is page-derived — clamp it before it leaves, even though the desktop
  // clamps it again.
  const limit = clampMaxChars(req.maxChars);
  if (streamKind === 'draft' && limit !== undefined) payload.maxChars = limit;
  try {
    const result = await getClient().answerAssist(payload, (delta) => {
      if (gen !== assistGeneration) return; // superseded — drop this late chunk
      assistBuffer = {
        ...assistBuffer,
        text: growAssistDraft(assistBuffer.text, delta),
        done: false,
        interrupted: false,
      };
      broadcastAssistProgress();
    });
    if (gen === assistGeneration) {
      assistBuffer = {
        ...assistBuffer,
        text: result.ok ? result.draft : assistBuffer.text,
        done: true,
        interrupted: !result.ok && assistBuffer.text.length > 0,
      };
      broadcastAssistProgress();
      // A finished run becomes a VERSION on its row (session-only, ADR-033
      // untouched). A refusal becomes the row's error instead, verbatim.
      if (rowId) await settleRowFromAssist(rowId, streamKind, result);
    }
    return { ok: true, kind: 'answerAssist', result };
  } catch (err) {
    if (gen === assistGeneration) {
      assistBuffer = {
        ...assistBuffer,
        done: true,
        interrupted: assistBuffer.text.length > 0,
      };
      broadcastAssistProgress();
    }
    throw err;
  }
}
