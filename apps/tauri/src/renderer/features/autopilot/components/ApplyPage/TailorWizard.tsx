import { AlertCircle, Check, ChevronLeft, ChevronRight, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { FormProvider, type UseFormReturn } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Button, cn, transition } from '@ajh/ui';

import type { TailorWizardState } from './lib/tailor-state';
import { StepJobAd } from './wizard-steps/StepJobAd';
import { StepModel } from './wizard-steps/StepModel';
import { StepOutput } from './wizard-steps/StepOutput';
import { StepResume } from './wizard-steps/StepResume';

const STEPS = ['jobAd', 'resume', 'output', 'model'] as const;

// Fields each step must pass before "Next" advances. Steps without an entry have
// no gate. The model step is gated on the global `canUse` flag (not RHF).
const STEP_FIELDS: Partial<Record<number, (keyof TailorWizardState)[]>> = {
  1: ['resume'],
};

interface Props {
  methods: UseFormReturn<TailorWizardState>;
  step: number;
  setStep: (step: number) => void;
  // Job-ad step data.
  jobDesc: string;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl: string;
  // Resume upload.
  onUpload: (file: File) => Promise<void>;
  uploading: boolean;
  // Global AI availability.
  canUse: boolean;
  reason?: string;
  // Submit the wizard → start generation.
  onGenerate: (values: TailorWizardState) => void;
}

/**
 * Full-page tailoring wizard. Mirrors the autopilot CreationWizard frame (same
 * numbered/checkmark step indicator, RHF FormProvider, AnimatePresence step
 * slide, Back/Next-or-Generate footer) but fills the page body instead of a
 * modal. The model availability gate is an inline banner (the model lives in the
 * global store, not RHF), matching CreationWizard's `canUse`-style guard.
 */
export function TailorWizard({
  methods,
  step,
  setStep,
  jobDesc,
  hasDesc,
  fetchingDesc,
  jobUrl,
  onUpload,
  uploading,
  canUse,
  reason,
  onGenerate,
}: Props) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);

  const handleNext = async () => {
    const fields = STEP_FIELDS[step];
    if (fields && !(await methods.trigger(fields))) return;
    setError(null);
    setStep(step + 1);
  };

  const handleGenerate = () => {
    if (!canUse) {
      setError(t('autopilot.apply.wizard.validation.selectModel'));
      return;
    }
    setError(null);
    void methods.handleSubmit(onGenerate)();
  };

  const isLast = step === STEPS.length - 1;

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Step indicators — same look as the CreationWizard for a cohesive feel. */}
      <div className="flex shrink-0 items-center gap-0 px-8">
        {STEPS.map((s, i) => (
          <div
            key={s}
            aria-current={i === step ? 'step' : undefined}
            className={cn(
              'flex flex-1 items-center justify-center gap-1.5 py-2.5 text-[11px] font-medium transition-colors',
              i === step
                ? 'text-brand-soft'
                : i < step
                  ? 'text-foreground/40'
                  : 'text-foreground/25'
            )}
          >
            {i < step ? (
              <Check size={10} className="text-emerald-400" aria-hidden="true" />
            ) : (
              <span className="flex h-4 w-4 items-center justify-center rounded-full border border-current text-[9px]">
                {i + 1}
              </span>
            )}
            {t(`autopilot.apply.wizard.steps.${s}`)}
          </div>
        ))}
      </div>

      {/* Step content */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-8 py-6">
        <FormProvider {...methods}>
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 16 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -16 }}
              transition={transition.normal}
              className="flex min-h-0 flex-1 flex-col"
            >
              {step === 0 && (
                <StepJobAd
                  jobDesc={jobDesc}
                  hasDesc={hasDesc}
                  fetchingDesc={fetchingDesc}
                  jobUrl={jobUrl}
                />
              )}
              {step === 1 && <StepResume onUpload={onUpload} uploading={uploading} />}
              {step === 2 && <StepOutput />}
              {step === 3 && <StepModel canUse={canUse} reason={reason} />}
            </motion.div>
          </AnimatePresence>
        </FormProvider>
      </div>

      {error && (
        <div className="mx-8 mb-3 flex shrink-0 items-center gap-2 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-300/80">
          <AlertCircle size={11} /> {error}
        </div>
      )}

      {/* Footer */}
      <div className="flex shrink-0 items-center justify-between border-t border-white/[0.06] px-8 py-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => step > 0 && setStep(step - 1)}
          disabled={step === 0}
          className="gap-1.5 text-foreground/50 hover:text-foreground/80 disabled:opacity-40"
        >
          <ChevronLeft size={13} /> {t('autopilot.apply.wizard.back')}
        </Button>
        {isLast ? (
          <Button variant="primary" size="sm" onClick={handleGenerate} className="gap-1.5">
            <Sparkles size={13} /> {t('autopilot.apply.wizard.generate')}
          </Button>
        ) : (
          <Button variant="primary" size="sm" onClick={() => void handleNext()} className="gap-1.5">
            {t('autopilot.apply.wizard.next')} <ChevronRight size={13} />
          </Button>
        )}
      </div>
    </div>
  );
}
