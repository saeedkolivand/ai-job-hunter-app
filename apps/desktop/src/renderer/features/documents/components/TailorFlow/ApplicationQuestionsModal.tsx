import { Check, Copy, HelpCircle, Plus, Sparkles, X } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useRef, useState } from 'react';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';
import { useTranslation } from '@ajh/translations';
import { Button, Input, ModalShell, Switch, useNotification } from '@ajh/ui';

import {
  RewritePopover,
  type RewriteTarget,
} from '@/components/generation/EditableOutput/RewritePopover';
import { getSelectionOffsets } from '@/lib/selection-offsets';
import { COPY_FEEDBACK_MS } from '@/lib/timings';

import { MAX_CUSTOM_QUESTION_LEN } from './useApplicationAnswers';

/** A rewrite frozen at trigger time — the splice range + snapshot answer it
 *  should be spliced back into on Accept (mirrors EditableOutput's FrozenRange). */
interface FrozenAnswer {
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

interface Props {
  selected: Set<string>;
  toggle: (id: string) => void;
  /** Opt-in per-question web search (off by default) — see `useApplicationAnswers`. */
  searchWeb: boolean;
  setSearchWeb: (next: boolean) => void;
  custom: { id: string; question: string }[];
  addCustom: (text: string) => void;
  removeCustom: (id: string) => void;
  answers: Record<string, string>;
  generating: boolean;
  error: string | null;
  generate: () => void;
  canGenerate: boolean;
  onClose: () => void;
  /** Model string for the rewrite popover (same source as the rest of TailorFlow). */
  model: string;
  /** Document/answer language — drives the rewrite locale. Defaults to 'en'. */
  locale?: string;
  /** Update a single answer text and persist (called on rewrite accept). */
  updateAnswer: (id: string, text: string) => Promise<void>;
  /** Revert a single answer to a previous text WITHOUT persisting (for rollback on save failure). */
  revertAnswer: (id: string, prev: string) => void;
}

/**
 * "Application questions" assistant lifted out of the cramped results stack into a
 * button-triggered modal (mirrors {@link ReferralModal}). The stateful hook lives
 * in {@link ApplyPage} and is passed in as props, so closing the modal unmounts
 * this body WITHOUT losing the user's picks/answers or interrupting generation.
 * The user picks from a curated list and the app drafts résumé-grounded answers.
 */
export function ApplicationQuestionsModal({
  selected,
  toggle,
  searchWeb,
  setSearchWeb,
  custom,
  addCustom,
  removeCustom,
  answers,
  generating,
  error,
  generate,
  canGenerate,
  onClose,
  model,
  locale = 'en',
  updateAnswer,
  revertAnswer,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  // The frozen rewrite (one at a time) — null when no popover is open.
  const [frozen, setFrozen] = useState<FrozenAnswer | null>(null);
  // Answer <p> elements keyed by question id — read to compute selection offsets.
  const answerRefs = useRef<Record<string, HTMLParagraphElement | null>>({});
  // Tracks the latest optimistically-written value per answer id. Set
  // SYNCHRONOUSLY in acceptRewrite (before the async save) so the .catch
  // guard never races against a React render cycle.
  const pendingRewriteRef = useRef<Record<string, string>>({});

  const copy = async (id: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), COPY_FEEDBACK_MS);
  };

  const submitCustom = () => {
    if (!draft.trim()) return;
    addCustom(draft);
    setDraft('');
  };

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

  // Shared answer + Copy + Rewrite block — reused by predefined and custom rows.
  const answerBlock = (id: string, answer: string) => (
    <div className="px-2 pb-2 pl-7">
      <div className="relative rounded-md border border-[var(--border-clear)] bg-card px-2.5 py-2">
        <p
          ref={(el) => {
            answerRefs.current[id] = el;
          }}
          className="select-text whitespace-pre-wrap text-[11px] leading-relaxed text-foreground/70"
        >
          {answer}
        </p>
        {/* Action row — Rewrite (primary affordance with label) + Copy */}
        <div className="mt-1.5 flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            type="button"
            // Keep the live text selection alive through the click — a bare click
            // would let the browser collapse it before onClick reads it.
            onMouseDown={(e) => e.preventDefault()}
            onClick={(e) => openRewrite(id, e.currentTarget)}
            title={t('autopilot.apply.questions.rewrite')}
            aria-label={t('autopilot.apply.questions.rewriteAriaLabel')}
            className="h-auto gap-1 px-1.5 py-0.5 text-[11px] text-brand-soft"
          >
            <Sparkles size={11} />
            {t('autopilot.apply.questions.rewrite')}
          </Button>
          <Button
            variant="unstyled"
            type="button"
            onClick={() => void copy(id, answer)}
            title={t('autopilot.apply.questions.copy')}
            aria-label={t('autopilot.apply.questions.copy')}
            className="rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
          >
            {copiedId === id ? <Check size={11} /> : <Copy size={11} />}
          </Button>
        </div>

        {/* Rewrite popover — portals to document.body and fixed-anchors off the
            trigger (see RewritePopover's `anchorEl`), so it escapes this card's
            clipping ancestors (ModalShell's overflow-hidden panel / overflow-y-auto
            body) instead of rendering — and clipping — inline. */}
        <AnimatePresence>
          {frozen?.id === id && (
            <RewritePopover
              target={frozen.target}
              docType="application-answer"
              model={model}
              locale={locale}
              anchorEl={frozen.anchorEl}
              onAccept={acceptRewrite}
              onClose={closeRewrite}
            />
          )}
        </AnimatePresence>
      </div>
    </div>
  );

  return (
    <ModalShell
      open
      onClose={onClose}
      maxWidth="max-w-lg"
      // Second-layer modal opened from the ApplyPage — sit above the default
      // modal layer (600) so it never renders under its parent.
      zIndex={650}
      ariaLabelledby="application-questions-modal-title"
      header={
        <div className="flex items-start justify-between gap-3 border-b border-[var(--border-clear)] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <HelpCircle size={14} className="shrink-0 text-brand-soft" />
              <span
                id="application-questions-modal-title"
                className="truncate text-sm font-semibold text-foreground/85"
              >
                {t('autopilot.apply.questions.title')}
              </span>
              {selected.size > 0 && (
                <span className="shrink-0 rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
                  {selected.size}
                </span>
              )}
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {t('autopilot.apply.questions.hint')}
            </div>
          </div>
          <Button
            onClick={onClose}
            aria-label={t('autopilot.referral.close')}
            className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
          >
            <X size={16} />
          </Button>
        </div>
      }
    >
      {/* Body */}
      <div className="space-y-2 px-5 py-4">
        <div className="rounded-md bg-card px-2.5 py-2">
          <Switch
            checked={searchWeb}
            onCheckedChange={setSearchWeb}
            disabled={generating}
            size="sm"
            label={t('autopilot.apply.questions.searchWeb.label')}
            description={t('autopilot.apply.questions.searchWeb.hint')}
          />
        </div>

        <div className="space-y-1.5">
          {APPLICATION_QUESTIONS.map((q) => {
            const answer = answers[q.id];
            return (
              <div key={q.id} className="rounded-md bg-card">
                <label className="flex cursor-pointer items-start gap-2 px-2 py-1.5">
                  <input
                    type="checkbox"
                    checked={selected.has(q.id)}
                    onChange={() => toggle(q.id)}
                    className="mt-0.5 accent-brand"
                  />
                  <span className="text-[11px] text-foreground/75">{q.question}</span>
                </label>
                {answer && answerBlock(q.id, answer)}
              </div>
            );
          })}
        </div>

        {/* Your own questions */}
        <div className="space-y-1.5 pt-1">
          <div className="px-2 text-[10px] font-medium uppercase tracking-wide text-foreground/40">
            {t('autopilot.apply.questions.customLabel')}
          </div>

          {custom.map((q) => {
            const answer = answers[q.id];
            return (
              <div key={q.id} className="rounded-md bg-card">
                <div className="flex items-start gap-2 px-2 py-1.5">
                  <span className="flex-1 text-[11px] text-foreground/75">{q.question}</span>
                  <Button
                    variant="unstyled"
                    type="button"
                    onClick={() => removeCustom(q.id)}
                    aria-label={t('autopilot.apply.questions.remove')}
                    className="shrink-0 rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                  >
                    <X size={12} />
                  </Button>
                </div>
                {answer && answerBlock(q.id, answer)}
              </div>
            );
          })}

          <div className="flex items-center gap-1.5 px-2">
            <Input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  submitCustom();
                }
              }}
              placeholder={t('autopilot.apply.questions.customPlaceholder')}
              maxLength={MAX_CUSTOM_QUESTION_LEN}
              className="h-8 flex-1 text-[11px]"
            />
            <Button
              type="button"
              onClick={submitCustom}
              disabled={!draft.trim()}
              className="h-8 shrink-0 px-2"
            >
              <Plus size={12} />
              {t('autopilot.apply.questions.add')}
            </Button>
          </div>
        </div>

        {error && <p className="text-[11px] text-red-300/80">{error}</p>}

        <Button
          variant="primary"
          loading={generating}
          disabled={!canGenerate || generating}
          onClick={() => void generate()}
          className="w-full justify-center"
        >
          {!generating && <Sparkles size={12} />}
          {generating
            ? t('autopilot.apply.questions.generating')
            : t('autopilot.apply.questions.generate')}
        </Button>
      </div>
    </ModalShell>
  );
}
