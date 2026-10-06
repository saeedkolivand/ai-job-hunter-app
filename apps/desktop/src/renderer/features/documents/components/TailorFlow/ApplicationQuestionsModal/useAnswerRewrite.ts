import { useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import type { RewriteTarget } from '@/components/generation/EditableOutput/RewritePopover';
import { getSelectionOffsets } from '@/lib/selection-offsets';

/** A rewrite frozen at trigger time — the splice range + snapshot answer it
 *  should be spliced back into on Accept (mirrors EditableOutput's FrozenRange). */
export interface FrozenAnswer {
  id: string;
  start: number;
  end: number;
  /** The answer string at freeze time — accept splices against this, not the
   *  live `answers[id]`, so a stray write in-between can't shift the offsets. */
  snapshot: string;
  target: RewriteTarget;
  /** The Rewrite button that opened this popover — anchors the portaled popover
   *  and reclaims focus when it closes. */
  anchorEl: HTMLElement;
}

interface Params {
  answers: Record<string, string>;
  /** Update a single answer text and persist (called on rewrite accept). */
  updateAnswer: (id: string, text: string) => Promise<void>;
  /** Revert a single answer to a previous text WITHOUT persisting (for rollback on save failure). */
  revertAnswer: (id: string, prev: string) => void;
}

/** The one-at-a-time inline rewrite of an answer: freeze the selection, then splice + persist on accept. */
export function useAnswerRewrite({ answers, updateAnswer, revertAnswer }: Params) {
  const { t } = useTranslation();
  const notify = useNotification();
  // The frozen rewrite (one at a time) — null when no popover is open.
  const [frozen, setFrozen] = useState<FrozenAnswer | null>(null);
  // Answer <p> elements keyed by question id — read to compute selection offsets.
  const answerRefs = useRef<Record<string, HTMLParagraphElement | null>>({});
  // Tracks the latest optimistically-written value per answer id. Set
  // SYNCHRONOUSLY in acceptRewrite (before the async save) so the .catch
  // guard never races against a React render cycle.
  const pendingRewriteRef = useRef<Record<string, string>>({});

  // Capture the live selection inside the answer's <p> (if any) and freeze it —
  // splice range + surrounding context — so the rewrite targets just the
  // selected span. Falls back to the whole answer when nothing is selected.
  const openRewrite = (id: string, trigger: HTMLElement) => {
    const answer = answers[id] ?? '';
    const container = answerRefs.current[id];
    const offsets = container ? getSelectionOffsets(container) : null;
    const start = offsets?.start ?? 0;
    const end = offsets?.end ?? answer.length;
    setFrozen({
      id,
      start,
      end,
      snapshot: answer,
      anchorEl: trigger,
      target: {
        selection: answer.slice(start, end),
        before: answer.slice(0, start),
        after: answer.slice(end),
      },
    });
  };
  const closeRewrite = () => {
    const trigger = frozen?.anchorEl;
    setFrozen(null);
    trigger?.focus();
  };
  // Close the popover immediately (never leave the user stuck), then fire the
  // persist. On failure: only revert if the answer hasn't been superseded by
  // a second rewrite that was accepted while this save was in-flight.
  // pendingRewriteRef is set SYNCHRONOUSLY here, so the guard is safe even if
  // the rejection arrives before React flushes the optimistic re-render.
  const acceptRewrite = (replacement: string) => {
    if (!frozen) return;
    const { id, start, end, snapshot } = frozen;
    const prev = answers[id] ?? '';
    const next = snapshot.slice(0, start) + replacement + snapshot.slice(end);
    setFrozen(null);
    pendingRewriteRef.current[id] = next; // synchronous — latest-wins sentinel
    updateAnswer(id, next)
      .then(() => {
        // Still current — clear the sentinel so it doesn't linger forever.
        if (pendingRewriteRef.current[id] === next) delete pendingRewriteRef.current[id];
      })
      .catch(() => {
        // Only revert/toast if this save is still the current one — a
        // superseded rewrite (a later accept already overwrote the sentinel)
        // failing shouldn't surface a stale "save failed" toast or clobber
        // the newer, already-displayed answer.
        if (pendingRewriteRef.current[id] === next) {
          revertAnswer(id, prev);
          notify.error({ message: t('autopilot.apply.questions.rewriteSaveError') });
        }
      });
  };

  return { frozen, answerRefs, openRewrite, closeRewrite, acceptRewrite };
}
