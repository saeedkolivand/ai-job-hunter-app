import { AlertCircle, Check, ChevronLeft, ChevronRight, X, Zap } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useState } from 'react';

import type { Autopilot } from '@ajh/shared';
import { Button, cn, transition } from '@ajh/ui';

import { StepAction } from '@/features/autopilot/components/wizard-steps/StepAction';
import { StepFilter } from '@/features/autopilot/components/wizard-steps/StepFilter';
import { StepSchedule } from '@/features/autopilot/components/wizard-steps/StepSchedule';
import { StepTarget } from '@/features/autopilot/components/wizard-steps/StepTarget';
import { buildDefaults } from '@/features/autopilot/lib/wizard-state';
import type { SetFn, WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';
import { useCreateAutopilot, useJobPreferences, useUpdateAutopilot } from '@/services';
import { useSessionStore } from '@/store/session-store';

import { WizardBackdrop } from './WizardBackdrop';

interface CreationWizardProps {
  onDone(ap: Autopilot): void;
  onCancel(): void;
}

const STEPS = ['Target', 'Filter', 'Action', 'Schedule'] as const;

export function CreationWizard({ onDone, onCancel }: CreationWizardProps) {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();

  const { autopilot, setAutopilot } = useSessionStore();
  const { wizardStep: step, wizardForm, editingId } = autopilot;
  const editing = editingId !== null;
  const setStep = (v: number) => setAutopilot({ wizardStep: v });
  const setWizardForm = useCallback(
    (v: WizardState) => setAutopilot({ wizardForm: v }),
    [setAutopilot]
  );
  const form = wizardForm ?? buildDefaults(jobPrefs);
  const [error, setError] = useState<string | null>(null);
  const createAutopilot = useCreateAutopilot();
  const updateAutopilot = useUpdateAutopilot();
  const saving = createAutopilot.isPending || updateAutopilot.isPending;

  // Initialize form in the store on first open (when wizardForm is null)
  useEffect(() => {
    if (!wizardForm) {
      setWizardForm(buildDefaults(jobPrefs));
    }
  }, [wizardForm, jobPrefs, setWizardForm]);

  // Track which fields were pre-filled from settings so we can show a hint.
  // Suppressed when editing — those values come from the autopilot, not settings.
  const prefilledFields = {
    location: !editing && !!jobPrefs?.location,
    keywords: !editing && (jobPrefs?.techStack?.length ?? 0) > 0,
  };

  const set: SetFn = <K extends keyof WizardState>(k: K, v: WizardState[K]) =>
    setWizardForm({ ...form, [k]: v });

  const canNext = () => {
    if (step === 0)
      return form.board && form.query.trim().length > 1 && form.name.trim().length > 1;
    return true;
  };

  const save = async () => {
    setError(null);
    const payload = {
      name: form.name,
      target: {
        board: form.board,
        query: form.query,
        location: form.location || undefined,
        workType: form.workType !== 'any' ? form.workType : undefined,
        pages: form.pages,
        dateFilter: form.dateFilter || undefined,
      },
      filter: {
        minMatchScore: form.minMatchScore,
        keywords: form.keywords
          ? form.keywords
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean)
          : undefined,
        excludeKeywords: form.excludeKeywords
          ? form.excludeKeywords
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean)
          : undefined,
      },
      resumeText: form.resumeText || undefined,
      action: form.action,
      coverLetter: form.coverLetter || undefined,
      autoSubmit: form.autoSubmit,
      schedule: form.schedule,
    };
    try {
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
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={transition.fast}
      className="absolute inset-0 z-[var(--z-modal)] flex items-center justify-center"
    >
      <WizardBackdrop />

      {/* Modal glass panel */}
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 14 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: 14 }}
        transition={transition.relaxed}
        style={{
          /* Denser, milkier glass — not transparent tinted window */
          background: 'rgba(14, 14, 26, 0.88)',
          backdropFilter: 'blur(32px) saturate(180%)',
          WebkitBackdropFilter: 'blur(32px) saturate(180%)',
          border: '1px solid rgba(255,255,255,0.13)',
          boxShadow: [
            /* purple accent ring */
            '0 0 0 1px rgba(168,85,247,0.15)',
            /* deep lift shadow */
            '0 40px 100px rgba(0,0,0,0.75)',
            '0 12px 40px rgba(0,0,0,0.5)',
            /* top edge highlight — the classic Apple glass line */
            'inset 0 1px 0 rgba(255,255,255,0.13)',
            /* bottom edge dim */
            'inset 0 -1px 0 rgba(0,0,0,0.3)',
          ].join(', '),
        }}
        className="relative w-full max-w-xl rounded-2xl overflow-hidden"
      >
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
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 16 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -16 }}
              transition={transition.normal}
            >
              {step === 0 && <StepTarget form={form} set={set} prefilled={prefilledFields} />}
              {step === 1 && <StepFilter form={form} set={set} prefilled={prefilledFields} />}
              {step === 2 && <StepAction form={form} set={set} />}
              {step === 3 && <StepSchedule form={form} set={set} />}
            </motion.div>
          </AnimatePresence>
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
              disabled={!canNext()}
              onClick={() => setStep(step + 1)}
              className="transition-all duration-150 ease-out"
            >
              {t('autopilot.wizard.next')} <ChevronRight size={13} />
            </Button>
          ) : (
            <Button
              variant="glass"
              size="sm"
              loading={saving}
              onClick={() => void save()}
              className="transition-all duration-150 ease-out"
            >
              {!saving && <Zap size={13} />}{' '}
              {t(editing ? 'autopilot.wizard.save' : 'autopilot.wizard.create')}
            </Button>
          )}
        </div>
      </motion.div>
    </motion.div>
  );
}
