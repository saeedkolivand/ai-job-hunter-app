import { useEffect, useId, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ModalShell, StepDots } from '@ajh/ui';

import { useModelInspections, useSpendSummary, useStageOverrides } from '@/services';
import { useAiProviderConfig } from '@/store/preferences-store';

import { ADVISOR_STEPS } from './steps-config';
import type { AdvisorContext } from './types';

interface Props {
  open: boolean;
  onClose: () => void;
  activeProvider?: string;
  activeModel?: string;
  /** Installed local model names, from the same list the provider rows use. */
  installedModels: string[];
}

/**
 * The model advisor — a guided read of what is installed, whether it fits the
 * stage budgets, what to change, and which combinations are known to go wrong.
 *
 * Ollama-focused by design: the questions it answers (does the window fit, is
 * the KV cache in VRAM, does this model think for ten minutes) are local-model
 * questions. It never writes the user's Ollama server configuration, and it
 * never touches web search — that is a separate axis from the AI provider
 * (ADR-0023) and conflating them here would be the same category error.
 *
 * The only thing it can WRITE is a per-stage override, through the same
 * `setStageOverride` path the Settings section uses, and only when the user
 * clicks accept.
 */
export function ModelAdvisor({
  open,
  onClose,
  activeProvider,
  activeModel,
  installedModels,
}: Props) {
  const { t } = useTranslation();
  const [stepIndex, setStepIndex] = useState(0);
  const titleId = useId();
  const stepRef = useRef<HTMLDivElement | null>(null);

  const inspections = useModelInspections(open ? installedModels : []);
  const { data: overrides = {} } = useStageOverrides();
  const { data: spend } = useSpendSummary();
  const zustand = useAiProviderConfig();

  // Move focus to the new step's container so a keyboard/screen-reader user
  // lands ON the content that just changed instead of keeping focus on the
  // "Next" button while the panel silently swaps underneath it.
  useEffect(() => {
    if (open) stepRef.current?.focus();
  }, [open, stepIndex]);

  const step = ADVISOR_STEPS[stepIndex] ?? ADVISOR_STEPS[0];
  if (!step) return null;
  const Current = step.component;

  const ctx: AdvisorContext = {
    activeProvider,
    activeModel,
    installedModels,
    inspections: inspections.byModel,
    inspectionsPending: inspections.isPending,
    overrides,
    effort: activeProvider ? zustand?.providers?.[activeProvider]?.effort : undefined,
    thinkingByModel: spend?.thinkingByModel ?? [],
  };

  const isLast = stepIndex === ADVISOR_STEPS.length - 1;
  const close = () => {
    setStepIndex(0);
    onClose();
  };

  return (
    <ModalShell
      open={open}
      onClose={close}
      maxWidth="max-w-xl"
      ariaLabelledby={titleId}
      header={
        <div className="border-b border-foreground/10 px-5 py-4">
          <h2 id={titleId} className="text-sm font-semibold text-foreground/90">
            {t('settings.ai.advisor.title')}
          </h2>
          <p className="mt-1 text-xs text-foreground/45">
            {t(`settings.ai.advisor.${step.id}.title`)}
          </p>
        </div>
      }
      footer={
        <div className="flex items-center justify-between gap-3 border-t border-foreground/10 px-5 py-3">
          <StepDots currentStep={stepIndex} totalSteps={ADVISOR_STEPS.length} />
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              disabled={stepIndex === 0}
              onClick={() => setStepIndex((i) => Math.max(0, i - 1))}
            >
              {t('settings.ai.advisor.back')}
            </Button>
            <Button variant="glass" onClick={() => (isLast ? close() : setStepIndex((i) => i + 1))}>
              {isLast ? t('settings.ai.advisor.done') : t('settings.ai.advisor.next')}
            </Button>
          </div>
        </div>
      }
    >
      <div ref={stepRef} tabIndex={-1} aria-labelledby={titleId} className="px-5 py-4 outline-none">
        <Current ctx={ctx} />
      </div>
    </ModalShell>
  );
}
