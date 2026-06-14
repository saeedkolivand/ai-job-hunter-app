import { Wand2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, IconBadge, Skeleton, StepDots } from '@ajh/ui';

import { ThinkingBubble } from '@/components/generation/ThinkingBubble';

import type { TailorTarget } from './useTailorGeneration';

interface Props {
  target: TailorTarget;
  phase: 'idle' | 'analyzing' | 'resume' | 'cover';
  phaseLabel: string;
  thinking: string;
  output: string;
  onCancel: () => void;
}

/**
 * Streaming stage: a hero icon + dot progress bar (analyze → resume/cover) + the
 * live phase caption, the model's reasoning bubble, and the streaming document
 * text. While output is empty the live region shows skeleton bars so the area
 * reads as actively generating. Cancel aborts the in-flight run.
 *
 * `both` runs all three phases (analyze, resume, cover → 3 dots). A single-doc
 * target skips one phase (2 dots): analyze=0 and the doc phase clamps to 1, so a
 * cover-only run lands on dot 1 (not 2, which would be out of range).
 */
export function GeneratingPanel({ target, phase, phaseLabel, thinking, output, onCancel }: Props) {
  const { t } = useTranslation();

  const totalSteps = target === 'both' ? 3 : 2;
  const currentStep =
    phase === 'analyzing'
      ? 0
      : target === 'both'
        ? phase === 'resume'
          ? 1
          : 2
        : // single-doc: the one doc phase is the second (and last) dot
          1;

  return (
    <div className="flex h-full min-h-0 flex-col px-8 py-6">
      {/* Hero: anchors the empty/early phase so the stage reads as active work */}
      <div className="mx-auto mt-2 flex w-full max-w-2xl shrink-0 flex-col items-center text-center">
        <IconBadge icon={Wand2} size="lg" shape="circle" className="animate-pulse" />
        <StepDots currentStep={currentStep} totalSteps={totalSteps} className="mb-2 mt-4" />
        <p className="text-[11px] font-medium text-brand-soft">{phaseLabel}</p>
        <p className="mt-1 text-[11px] text-foreground/40">{t('autopilot.apply.generatingHint')}</p>
      </div>

      <div className="mx-auto mt-5 flex w-full max-w-2xl min-h-0 flex-1 flex-col overflow-y-auto">
        <ThinkingBubble thinking={thinking} done={false} />
        {output ? (
          <div className="select-text flex-1 whitespace-pre-wrap rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
            {output}
          </div>
        ) : (
          <div className="space-y-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-3">
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
