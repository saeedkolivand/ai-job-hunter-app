import { AlertCircle, Check, ChevronLeft, ChevronRight, X, Zap } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { FormProvider, useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import type { Autopilot } from '@ajh/shared';
import { Button, cn, ModalShell, transition } from '@ajh/ui';

import { StepAction } from '@/features/autopilot/components/wizard-steps/StepAction';
import { StepFilter } from '@/features/autopilot/components/wizard-steps/StepFilter';
import { StepSchedule } from '@/features/autopilot/components/wizard-steps/StepSchedule';
import { StepTarget } from '@/features/autopilot/components/wizard-steps/StepTarget';
import { autopilotWizardSchema } from '@/features/autopilot/lib/schema';
import { buildDefaults, wizardStateToPayload } from '@/features/autopilot/lib/wizard-state';
import type { WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';
import { useCreateAutopilot, useJobPreferences, useUpdateAutopilot } from '@/services';
import { useSessionStore } from '@/store/session-store';

interface CreationWizardProps {
  onDone(ap: Autopilot): void;
  onCancel(): void;
}

const STEPS = ['Target', 'Filter', 'Action', 'Schedule'] as const;

// Fields each step must pass before "Next" advances. Steps without an entry have
// no gate (their controls are bounded, so they can't hold invalid input).
const STEP_FIELDS: Partial<Record<number, (keyof WizardState)[]>> = {
  0: ['name', 'board', 'query'],
};

export function CreationWizard({ onDone, onCancel }: CreationWizardProps) {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();

  const { autopilot, setAutopilot } = useSessionStore();
  const { wizardStep: step, wizardForm, editingId } = autopilot;
  const editing = editingId !== null;
  const setStep = (v: number) => setAutopilot({ wizardStep: v });

  // RHF owns the live editing layer; the store's `wizardForm` is a one-shot seed
  // (an edit's snapshot or null on create). Seed `defaultValues` ONCE — never
  // write back. `buildDefaults` may be sparse if job preferences haven't loaded
  // yet; the effect below folds them in while the form is still pristine.
  const initialForm = useRef<WizardState>(wizardForm ?? buildDefaults(jobPrefs));
  const methods = useForm<WizardState>({
    defaultValues: initialForm.current,
    resolver: zodResolver(autopilotWizardSchema),
    mode: 'onChange',
  });

  const [error, setError] = useState<string | null>(null);
  const createAutopilot = useCreateAutopilot();
  const updateAutopilot = useUpdateAutopilot();
  const saving = createAutopilot.isPending || updateAutopilot.isPending;

  // Create mode only: when job preferences arrive after mount, fold their
  // prefilled values into the still-pristine form once. Skip when editing (the
  // seed came from the autopilot) or once the user has started typing.
  const seededPrefs = useRef(false);
  useEffect(() => {
    if (editing || seededPrefs.current || !jobPrefs) return;
    if (methods.formState.isDirty) {
      seededPrefs.current = true;
      return;
    }
    methods.reset(buildDefaults(jobPrefs));
    seededPrefs.current = true;
  }, [editing, jobPrefs, methods]);

  // Track which fields were pre-filled from settings so we can show a hint.
  // Suppressed when editing — those values come from the autopilot, not settings.
  const prefilledFields = {
    location: !editing && !!jobPrefs?.location,
    keywords: !editing && (jobPrefs?.techStack?.length ?? 0) > 0,
  };

  const handleNext = async () => {
    const fields = STEP_FIELDS[step];
    if (fields && !(await methods.trigger(fields))) {
      setError(t('autopilot.wizard.validation.missingFields'));
      return;
    }
    setError(null);
    setStep(step + 1);
  };

  const onValid = async (values: WizardState) => {
    setError(null);
    try {
      const payload = wizardStateToPayload(values);
      const ap =
        editingId !== null
          ? await updateAutopilot.mutateAsync({ id: editingId, ...payload })
          : await createAutopilot.mutateAsync(payload);
      onDone(ap);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t(editing ? 'autopilot.wizard.updateFailed' : 'autopilot.wizard.createFailed')
      );
    }
  };

  return (
    <ModalShell open onClose={onCancel} maxWidth="max-w-xl">
      <div className="relative w-full overflow-hidden">
        {/* Wizard header */}
        <div className="flex items-center justify-between border-white/[0.1] px-6 py-4">
          <div className="flex items-center gap-2">
            <Zap size={14} className="text-brand-soft" />
            <span className="text-sm font-semibold text-foreground/80">
              {t(editing ? 'autopilot.wizard.editTitle' : 'autopilot.wizard.title')}
            </span>
          </div>
          <Button
            onClick={onCancel}
            className="text-foreground/30 hover:text-foreground/60 transition-colors h-auto bg-transparent border-transparent p-0"
          >
            <X size={16} />
          </Button>
        </div>

        {/* Step indicators */}
        <div className="flex items-center gap-0">
          {STEPS.map((s, i) => (
            <div
              key={s}
              className={cn(
                'flex-1 flex items-center justify-center gap-1.5 py-2.5 text-[11px] font-medium transition-colors',
                i === step
                  ? 'border-brand text-brand-soft'
                  : i < step
                    ? 'border-emerald-400/50 text-foreground/40'
                    : 'border-transparent text-foreground/25'
              )}
            >
              {i < step ? (
                <Check size={10} className="text-emerald-400" />
              ) : (
                <span className="h-4 w-4 rounded-full border border-current flex items-center justify-center text-[9px]">
                  {i + 1}
                </span>
              )}
              {t(`autopilot.wizard.steps.${i}`)}
            </div>
          ))}
        </div>

        {/* Step content */}
        <div className="px-6 py-6 min-h-[320px]">
          <FormProvider {...methods}>
            <AnimatePresence mode="wait">
              <motion.div
                key={step}
                initial={{ opacity: 0, x: 16 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -16 }}
                transition={transition.normal}
              >
                {step === 0 && <StepTarget prefilled={prefilledFields} />}
                {step === 1 && <StepFilter prefilled={prefilledFields} />}
                {step === 2 && <StepAction />}
                {step === 3 && <StepSchedule />}
              </motion.div>
            </AnimatePresence>
          </FormProvider>
        </div>

        {error && (
          <div className="mx-6 mb-4 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-300/80 flex items-center gap-2">
            <AlertCircle size={11} /> {error}
          </div>
        )}

        {/* Wizard footer */}
        <div className="flex items-center justify-between border-t border-white/[0.1] px-6 py-4">
          <Button
            onClick={() => (step > 0 ? setStep(step - 1) : onCancel())}
            className="flex items-center gap-1.5 text-xs text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
          >
            <ChevronLeft size={13} />{' '}
            {step === 0 ? t('autopilot.wizard.cancel') : t('autopilot.wizard.back')}
          </Button>
          {step < STEPS.length - 1 ? (
            <Button
              variant="glass"
              size="sm"
              onClick={() => void handleNext()}
              className="transition-all duration-150 ease-out"
            >
              {t('autopilot.wizard.next')} <ChevronRight size={13} />
            </Button>
          ) : (
            <Button
              variant="glass"
              size="sm"
              loading={saving}
              onClick={() => void methods.handleSubmit(onValid)()}
              className="transition-all duration-150 ease-out"
            >
              {!saving && <Zap size={13} />}{' '}
              {t(editing ? 'autopilot.wizard.save' : 'autopilot.wizard.create')}
            </Button>
          )}
        </div>
      </div>
    </ModalShell>
  );
}
