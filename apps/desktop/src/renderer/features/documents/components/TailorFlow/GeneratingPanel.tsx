import { Check, Loader2 } from 'lucide-react';

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
  onCancel: () => void;
}

/**
 * Streaming stage: a 4-row step checklist (Analyze → Generate → Validate →
 * Remove AI signs), each row showing a one-line description and a
 * done-checkmark once passed; the active row gets a subtle spinner and the
 * pipeline's own finer stage name as a caption. Below it, the model's
 * reasoning bubble and the streaming document text — same as before. Cancel
 * aborts the in-flight run.
 *
 * Modeled on `TailorWizard`'s own numbered-circle → checkmark step header
 * (`TailorWizard.tsx`), so the apply flow reads as one continuous wizard
 * rather than two different step-indicator styles back to back.
 */
export function GeneratingPanel({ currentStep, stageLabel, thinking, output, onCancel }: Props) {
  const { t } = useTranslation();

  return (
    <div className="flex h-full min-h-0 flex-col px-8 py-6">
      <ul
        aria-label={t('pipeline.step.title')}
        className="mx-auto w-full max-w-2xl shrink-0 space-y-1.5"
      >
        {PIPELINE_STEP_KEYS.map((key, i) => {
          const done = i < currentStep;
          const active = i === currentStep;
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
                className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border border-current text-[9px] text-foreground/40"
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
                    active
                      ? 'text-foreground/90'
                      : done
                        ? 'text-foreground/50'
                        : 'text-foreground/35'
                  )}
                >
                  {t(`pipeline.step.${key}.label`)}
                </span>
                <span className="mt-0.5 block text-[10px] text-foreground/40">
                  {t(`pipeline.step.${key}.description`)}
                </span>
                {active && stageLabel && (
                  <span className="mt-0.5 block text-[9px] uppercase tracking-[0.14em] text-brand-soft/70">
                    {stageLabel}
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>

      <div className="mx-auto mt-5 flex w-full max-w-2xl min-h-0 flex-1 flex-col overflow-y-auto">
        <ThinkingBubble thinking={thinking} done={false} />
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
          className="border-red-400/20 text-red-300/80 hover:text-red-200"
        >
          {t('autopilot.apply.cancel')}
        </Button>
      </div>
    </div>
  );
}
