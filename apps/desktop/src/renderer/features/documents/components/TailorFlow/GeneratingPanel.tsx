import { Check, Loader2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Skeleton } from '@ajh/ui';

import { ThinkingBubble } from '@/components/generation/ThinkingBubble';

import { PIPELINE_STEP_KEYS } from './lib/pipeline-steps';

interface Props {
  /** 0-based index into `PIPELINE_STEP_KEYS` — everything before it is done,
   *  the entry at it is the active row, everything after is still to come. */
  currentStep: number;
  /** The CURRENT stage's own name, translated (`pipeline.stage.*`) — a finer
   *  caption under the active row than the fixed 4-step label alone gives. */
  stageLabel: string;
  thinking: string;
  output: string;
  /** Which document `output` is currently streaming — only `draft`/`cover_letter`
   *  emit deltas (see `use-resume-pipeline-session`'s module doc), so this never
   *  changes after the letter's first token lands. Labels the streaming pane so
   *  a résumé→letter swap mid-run doesn't read as the SAME document continuing. */
  streamingTarget: 'resume' | 'cover';
  /**
   * The run's own start timestamp (epoch ms), as recorded by the backend on
   * `pipeline_runs.started_at` — `null` only in the brief window before the
   * run record's first fetch lands (or if no run id exists yet). Anchoring
   * the elapsed caption on this rather than component-mount time is the
   * whole fix for #993/owner-report: the run keeps going on the backend
   * regardless of whether this panel is mounted, so a navigate-away-and-back
   * (which unmounts and reconnects the whole flow, not just this panel —
   * see `ApplicationDetailPage`'s `tab === 'documents'` gate) must read the
   * SAME elapsed value, not restart at 0:00.
   */
  runStartedAt: number | null;
  onCancel: () => void;
}

/** mm:ss since `since` (epoch ms). Plain text, so it keeps ticking under
 *  `prefers-reduced-motion` — nothing here is a CSS animation. */
function elapsedLabel(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

/**
 * Streaming stage: a 4-row step checklist (Analyze → Generate → Validate →
 * Remove AI signs), each row showing a done-checkmark once passed; the active
 * row gets a subtle spinner, the pipeline's own finer stage name, and an
 * elapsed-time caption (validate/humanize emit no stream deltas, so without a
 * moving number the panel reads as stalled for its longest steps). Below it,
 * the model's reasoning bubble (collapsed by default — this panel is tight on
 * vertical space) and the streaming document text, labelled with which
 * document is currently being written. Cancel aborts the in-flight run.
 *
 * Modeled on `TailorWizard`'s own numbered-circle → checkmark step header
 * (`TailorWizard.tsx`), so the apply flow reads as one continuous wizard
 * rather than two different step-indicator styles back to back — the active
 * row's label uses the same `text-brand-soft` affordance.
 */
export function GeneratingPanel({
  currentStep,
  stageLabel,
  thinking,
  output,
  streamingTarget,
  runStartedAt,
  onCancel,
}: Props) {
  const { t } = useTranslation();

  // Elapsed time since the RUN started — NOT reset on a step change, and NOT
  // reset on a remount (`runStartedAt` is the backend's own persisted
  // timestamp, read fresh from the run record every mount). Falls back to
  // mount time only for the brief window before that record's first fetch
  // lands; `mountFallback` is captured once and never reassigned, so it
  // can't itself become a second "resets on remount" bug.
  const [mountFallback] = useState(() => Date.now());
  const anchor = runStartedAt ?? mountFallback;
  const [elapsedSec, setElapsedSec] = useState(0);
  useEffect(() => {
    // Clamped at 0: `anchor` is a backend timestamp, so a clock-skewed host
    // can put it slightly ahead of this client's `Date.now()` even though
    // it passed the `> 0` guard on the way in — without the clamp that reads
    // as a negative "N total" caption instead of just starting at 0:00.
    const tick = () => setElapsedSec(Math.max(0, Math.floor((Date.now() - anchor) / 1000)));
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [anchor]);

  // One utterance per step TRANSITION — an aria-hidden icon is the only other
  // state cue on each row, so without this a screen-reader user hears
  // identical rows and no step change is ever announced.
  const previousStepRef = useRef(currentStep);
  const [announcement, setAnnouncement] = useState('');
  useEffect(() => {
    if (previousStepRef.current === currentStep) return;
    previousStepRef.current = currentStep;
    const key = PIPELINE_STEP_KEYS[currentStep];
    if (!key) {
      // currentStep reached PIPELINE_STEP_KEYS.length — every step just
      // finished. Previously a silent no-op: a screen-reader user heard
      // every step START (below) and never heard the run finish — the
      // owner's "mark it as done" ask, for non-visual users.
      setAnnouncement(t('pipeline.step.allDone'));
      return;
    }
    setAnnouncement(
      t('pipeline.step.announce', {
        step: t(`pipeline.step.${key}.label`),
        state: t('pipeline.step.state.active'),
      })
    );
  }, [currentStep, t]);

  return (
    <div className="flex h-full min-h-0 flex-col px-8 py-6">
      <ul
        aria-label={t('pipeline.step.title')}
        className="mx-auto w-full max-w-2xl shrink-0 space-y-1.5"
      >
        {PIPELINE_STEP_KEYS.map((key, i) => {
          const done = i < currentStep;
          const active = i === currentStep;
          const stateWord = t(
            `pipeline.step.state.${done ? 'done' : active ? 'active' : 'pending'}`
          );
          return (
            <li
              key={key}
              aria-current={active ? 'step' : undefined}
              className={cn(
                'flex items-start gap-2.5 rounded-lg border px-3 py-2 transition-colors',
                active ? 'border-brand/25 bg-brand/5' : 'border-[var(--border-clear)] bg-card'
              )}
            >
              <span
                aria-hidden="true"
                className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-current text-[9px] text-foreground/70"
              >
                {done ? (
                  <Check size={10} className="text-emerald-400" />
                ) : active ? (
                  <Loader2 size={10} className="animate-spin text-brand-soft" />
                ) : (
                  i + 1
                )}
              </span>
              <span className="min-w-0 flex-1">
                <span
                  className={cn(
                    'block text-[11px] font-medium',
                    active ? 'text-brand-soft' : done ? 'text-foreground/50' : 'text-foreground/55'
                  )}
                >
                  {t(`pipeline.step.${key}.label`)}
                  {/* Screen readers hear identical rows otherwise — the icon
                      above is aria-hidden and is the only other state cue. */}
                  <span className="sr-only"> — {stateWord}</span>
                </span>
                {active ? (
                  stageLabel && (
                    <span className="mt-0.5 block text-[10px] text-brand-soft">
                      {stageLabel} ·{' '}
                      {t('pipeline.step.elapsedTotal', { time: elapsedLabel(elapsedSec) })}
                    </span>
                  )
                ) : (
                  <span className="mt-0.5 block text-[10px] text-foreground/70">
                    {t(`pipeline.step.${key}.description`)}
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>

      {/* One utterance per step transition; the rows themselves are not a
          live region, so nothing is re-read when a later step activates. */}
      <span role="status" aria-live="polite" className="sr-only">
        {announcement}
      </span>

      <div className="mx-auto mt-5 flex w-full max-w-2xl min-h-0 flex-1 flex-col overflow-y-auto">
        <ThinkingBubble thinking={thinking} done={currentStep >= 2} defaultExpanded={false} />
        <span className="mb-1 block shrink-0 text-[10px] font-semibold uppercase tracking-[0.14em] text-foreground/70">
          {t(
            streamingTarget === 'cover'
              ? 'autopilot.apply.target.cover'
              : 'autopilot.apply.target.resume'
          )}
        </span>
        {output ? (
          <div className="select-text flex-1 whitespace-pre-wrap rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
            {output}
          </div>
        ) : (
          <div className="space-y-2 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-3">
            <Skeleton className="h-2.5 w-full" />
            <Skeleton className="h-2.5 w-11/12" />
            <Skeleton className="h-2.5 w-4/5" />
            <Skeleton className="h-2.5 w-5/6" />
            <Skeleton className="h-2.5 w-2/3" />
          </div>
        )}
      </div>

      <div className="mt-4 flex shrink-0 justify-center">
        <Button
          variant="glass"
          onClick={onCancel}
          className="border-red-400/20 text-red-300 hover:text-red-200"
        >
          {t('autopilot.apply.cancel')}
        </Button>
      </div>
    </div>
  );
}
