import { X } from 'lucide-react';
import { useId, useState } from 'react';

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
  const stepTitleId = useId();

  const inspections = useModelInspections(open ? installedModels : []);
  const { data: overrides = {} } = useStageOverrides();
  const { data: spend } = useSpendSummary();
  const zustand = useAiProviderConfig();

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
        <div className="flex items-start justify-between gap-3 border-b border-[var(--border-soft)] px-5 py-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-sm font-semibold text-foreground/90">
              {t('settings.ai.advisor.title')}
            </h2>
            {/* The counter is computed from the step list, so adding a step
                cannot leave eight translated strings claiming "of 4".
                `aria-live` carries the step change to a screen reader without
                yanking focus off the button the user just pressed. */}
            <h3 id={stepTitleId} aria-live="polite" className="mt-1 text-xs text-foreground/60">
              {t('settings.ai.advisor.stepCounter', {
                current: stepIndex + 1,
                total: ADVISOR_STEPS.length,
              })}{' '}
              — {t(`settings.ai.advisor.${step.id}.title`)}
            </h3>
          </div>
          {/* Reachable on every step, not only the last one. */}
          <Button
            variant="ghost"
            className="shrink-0"
            aria-label={t('settings.ai.advisor.close')}
            onClick={close}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </div>
      }
      footer={
        <div className="flex items-center justify-between gap-3 border-t border-[var(--border-soft)] px-5 py-3">
          <StepDots currentStep={stepIndex} totalSteps={ADVISOR_STEPS.length} className="my-0" />
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
      {/* A labelled region, NOT a tab stop.
       *
       * Focusing the container was the first attempt and it broke the trap:
       * `useFocusTrap` only intercepts Tab when the active element is the
       * first/last of its FOCUSABLE query, which excludes `[tabindex="-1"]`, so
       * Shift+Tab from a focused `-1` container landed on the page behind the
       * overlay. Making it `tabIndex={0}` fixed that but turned a prose panel
       * into an unexplained tab stop.
       *
       * Focus therefore stays on the control the user just pressed (Next/Back —
       * both real, both inside the trap) and the step change is ANNOUNCED
       * instead, via the polite live region on the heading above. */}
      <div role="group" aria-labelledby={stepTitleId} className="px-5 py-4">
        <Current ctx={ctx} />
      </div>
    </ModalShell>
  );
}
