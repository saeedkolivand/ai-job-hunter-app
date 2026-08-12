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
  /**
   * Staged-run progress, present only for a QUALITY-depth run: the live stage
   * counter straight off `pipeline:stage`. When it's here the dots track the
   * real pipeline (`index`/`total`) instead of the fast path's 2–3 document
   * phases, and `stageLabel` names the stage under them.
   *
   * The panel stays otherwise identical — including `output`, which for a
   * staged run is the draft stage's DISPLAY-ONLY stream. That stream ends
   * several stages before the run does (validation and up to two repair rounds
   * follow), so this panel is never the thing that decides a run is finished;
   * its host leaves the generating stage on the run record's status.
   */
  pipelineStep?: { current: number; total: number } | null;
  stageLabel?: string;
}

/**
 * When the live reasoning stream has this many characters AND dwarfs the
 * document by this ratio, the run is reasoning-heavy — a real gpt-oss:20b
 * session logged 1,039,821 chars of reasoning against 45,416 of answer (22.9:1)
 * and took 8–13 minutes per document, where a non-reasoning model did the same
 * work in 12–23 SECONDS. The floor keeps the hint off short, normal runs.
 *
 * ponytail: measured live rather than from a per-model table — a table would go
 * stale with every model release, and the ratio is the thing that matters
 * anyway. Upgrade path if this needs to be known BEFORE a run starts: persist
 * thinking tokens alongside `output_tokens` in the spend store.
 */
const REASONING_HEAVY_MIN_CHARS = 8_000;
const REASONING_HEAVY_RATIO = 5;

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
export function GeneratingPanel({
  target,
  phase,
  phaseLabel,
  thinking,
  output,
  onCancel,
  pipelineStep,
  stageLabel,
}: Props) {
  const { t } = useTranslation();

  // "Is it stuck?" is the question a multi-minute silent run provokes, and the
  // honest answer is visible right here in the stream.
  const reasoningHeavy =
    thinking.length >= REASONING_HEAVY_MIN_CHARS &&
    thinking.length > output.length * REASONING_HEAVY_RATIO;

  const fastTotalSteps = target === 'both' ? 3 : 2;
  const fastCurrentStep =
    phase === 'analyzing'
      ? 0
      : target === 'both'
        ? phase === 'resume'
          ? 1
          : 2
        : // single-doc: the one doc phase is the second (and last) dot
          1;

  // A staged run drives the dots off its own stage counter. `total` can be 0
  // for a run reconnected from a persisted trail (the stored events carry no
  // pipeline length), so fall back rather than render an empty dot row.
  const staged = pipelineStep && pipelineStep.total > 0 ? pipelineStep : null;
  const totalSteps = staged ? staged.total : fastTotalSteps;
  const currentStep = staged ? Math.min(staged.current, staged.total - 1) : fastCurrentStep;

  return (
    <div className="flex h-full min-h-0 flex-col px-8 py-6">
      {/* Hero: anchors the empty/early phase so the stage reads as active work */}
      <div className="mx-auto mt-2 flex w-full max-w-2xl shrink-0 flex-col items-center text-center">
        <IconBadge icon={Wand2} size="lg" shape="circle" className="animate-pulse" />
        <StepDots currentStep={currentStep} totalSteps={totalSteps} className="mb-2 mt-4" />
        <p className="text-[11px] font-medium text-brand-soft">{stageLabel || phaseLabel}</p>
        {staged && (
          <p className="mt-0.5 text-[10px] uppercase tracking-[0.16em] text-foreground/35">
            {t('pipeline.stageCounter', {
              current: staged.current + 1,
              total: staged.total,
            })}
          </p>
        )}
        <p className="mt-1 text-[11px] text-foreground/40">
          {reasoningHeavy
            ? t('autopilot.apply.reasoningHeavyHint')
            : t('autopilot.apply.generatingHint')}
        </p>
      </div>

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
