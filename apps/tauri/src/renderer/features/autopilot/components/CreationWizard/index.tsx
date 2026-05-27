import { AlertCircle, Check, ChevronLeft, ChevronRight, X, Zap } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useState } from 'react';

import type { Autopilot, JobPreferences } from '@ajh/shared';
import { Button } from '@ajh/ui';

import { StepAction } from '@/features/autopilot/components/wizard-steps/StepAction';
import { StepFilter } from '@/features/autopilot/components/wizard-steps/StepFilter';
import { StepSchedule } from '@/features/autopilot/components/wizard-steps/StepSchedule';
import { StepTarget } from '@/features/autopilot/components/wizard-steps/StepTarget';
import type { SetFn, WizardState } from '@/features/autopilot/types';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useCreateAutopilot, useJobPreferences } from '@/services';
import { useSessionStore } from '@/store/session-store';

interface CreationWizardProps {
  onDone(ap: Autopilot): void;
  onCancel(): void;
}

const STEPS = ['Target', 'Filter', 'Action', 'Schedule'] as const;

function buildDefaults(jobPrefs?: JobPreferences): WizardState {
  const validWorkType = ['remote', 'hybrid', 'on-site', 'any'] as const;
  return {
    name: '',
    board: 'linkedin',
    query: '',
    location: jobPrefs?.location ?? '',
    workType: validWorkType.includes(jobPrefs?.remote as (typeof validWorkType)[number])
      ? (jobPrefs?.remote as WizardState['workType'])
      : 'any',
    pages: 2,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: jobPrefs?.techStack?.map((t) => t.name).join(', ') ?? '',
    excludeKeywords: '',
    resumeText: '',
    action: 'save',
    coverLetter: '',
    autoSubmit: false,
    schedule: 'daily',
  };
}

export function CreationWizard({ onDone, onCancel }: CreationWizardProps) {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();

  const { autopilot, setAutopilot } = useSessionStore();
  const { wizardStep: step, wizardForm } = autopilot;
  const setStep = (v: number) => setAutopilot({ wizardStep: v });
  const setWizardForm = useCallback(
    (v: WizardState) => setAutopilot({ wizardForm: v }),
    [setAutopilot]
  );
  const form = wizardForm ?? buildDefaults(jobPrefs as JobPreferences | undefined);
  const [error, setError] = useState<string | null>(null);
  const createAutopilot = useCreateAutopilot();
  const saving = createAutopilot.isPending;

  // Initialize form in the store on first open (when wizardForm is null)
  useEffect(() => {
    if (!wizardForm) {
      setWizardForm(buildDefaults(jobPrefs as JobPreferences | undefined));
    }
  }, [wizardForm, jobPrefs, setWizardForm]);

  // Track which fields were pre-filled so we can show a hint
  const prefilledFields = {
    location: !!jobPrefs?.location,
    keywords: ((jobPrefs as JobPreferences | undefined)?.techStack?.length ?? 0) > 0,
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
    try {
      const ap = (await createAutopilot.mutateAsync({
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
      })) as Autopilot;
      onDone(ap);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('autopilot.wizard.createFailed'));
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
      {/*
       * BACKDROP LAYERING — reading order = bottom to top
       *
       * 1. blur layer     — destroys shape detail
       * 2. crush layer    — kills remaining contrast/readability
       * 3. bokeh blobs    — adds ambient atmospheric color
       * 4. vignette       — pulls focus to center
       * 5. modal panel    — the actual foreground
       */}

      {/* 1 — heavy blur. backdrop-filter alone preserves contrast,
              so we follow it with a dark crush layer. */}
      <div
        className="absolute inset-0"
        style={{
          backdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
          WebkitBackdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
        }}
      />

      {/* 2 — contrast crush: semi-opaque dark tint that makes the
              blurred background unreadable without going full black. */}
      <div className="absolute inset-0 bg-background/70" />

      {/* 3 — bokeh blobs: soft ambient color on top of the dark crush. */}
      <div className="absolute inset-0 pointer-events-none overflow-hidden">
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob1}
          className="absolute"
          style={{
            top: '-5%',
            left: '5%',
            width: 500,
            height: 500,
            background: 'radial-gradient(circle, rgba(168,85,247,0.18) 0%, transparent 65%)',
            filter: 'blur(48px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob2}
          className="absolute"
          style={{
            bottom: '0%',
            right: '5%',
            width: 420,
            height: 420,
            background: 'radial-gradient(circle, rgba(79,70,229,0.15) 0%, transparent 65%)',
            filter: 'blur(56px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob3}
          className="absolute"
          style={{
            top: '40%',
            left: '40%',
            width: 300,
            height: 300,
            background: 'radial-gradient(circle, rgba(192,38,211,0.07) 0%, transparent 65%)',
            filter: 'blur(72px)',
          }}
        />
      </div>

      {/* 4 — vignette: darkens screen edges, naturally centres the eye. */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            'radial-gradient(ellipse 80% 80% at 50% 50%, transparent 35%, rgba(0,0,0,0.65) 100%)',
        }}
      />

      {/* 5 — modal glass panel */}
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
              {t('autopilot.wizard.title')}
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
              {!saving && <Zap size={13} />} {t('autopilot.wizard.create')}
            </Button>
          )}
        </div>
      </motion.div>
    </motion.div>
  );
}
