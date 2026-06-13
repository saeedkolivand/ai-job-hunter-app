import { ChevronLeft, ChevronRight, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef } from 'react';
import { FormProvider, useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';

import { useTranslation } from '@ajh/translations';
import { Button, StepDots, transition } from '@ajh/ui';

import type { InterviewAnswers, TemplateId } from '@/lib/generate';
import { useSessionStore } from '@/store/session-store';

import { type BuilderForm, builderSchema } from '../../lib/schema';
import { StepContact } from '../wizard-steps/StepContact';
import { StepEducation } from '../wizard-steps/StepEducation';
import { StepExperience } from '../wizard-steps/StepExperience';
import { StepExtras } from '../wizard-steps/StepExtras';
import { StepReview } from '../wizard-steps/StepReview';
import { StepSkills } from '../wizard-steps/StepSkills';
import { StepSummary } from '../wizard-steps/StepSummary';
import { WizardStep } from '../WizardStep';

const TOTAL_STEPS = 7;

/** How long RHF edits settle before they are pushed into the Zustand slice. */
const SYNC_DEBOUNCE_MS = 350;

/**
 * Per-step vertical alignment inside the reading column. Short steps (Summary,
 * Skills) center against the viewport; growing steps (Contact, Experience,
 * Education, Extras, Review) flow from the top.
 */
const STEP_ALIGN: readonly ('center' | 'top')[] = [
  'top', // 0 Contact
  'center', // 1 Summary
  'top', // 2 Experience
  'top', // 3 Education
  'center', // 4 Skills
  'top', // 5 Extras
  'top', // 6 Review
];

/** Project the persisted answers onto the in-scope RHF form value (drops `fullName`). */
function toFormValues(answers: InterviewAnswers): BuilderForm {
  return {
    headline: answers.headline ?? '',
    summary: answers.summary ?? '',
    experience: (answers.experience ?? []).map((e) => ({
      title: e.title ?? '',
      company: e.company ?? '',
      location: e.location ?? '',
      startDate: e.startDate ?? '',
      endDate: e.endDate ?? '',
      current: e.current ?? false,
      bullets: e.bullets ?? [],
    })),
    education: (answers.education ?? []).map((e) => ({
      degree: e.degree ?? '',
      institution: e.institution ?? '',
      location: e.location ?? '',
      startDate: e.startDate ?? '',
      endDate: e.endDate ?? '',
      details: e.details ?? '',
    })),
    skills: answers.skills ?? [],
    projects: (answers.projects ?? []).map((p) => ({
      name: p.name ?? '',
      description: p.description ?? '',
      link: p.link ?? '',
    })),
    publications: (answers.publications ?? []).map((p) => ({
      title: p.title ?? '',
      venue: p.venue ?? '',
      year: p.year ?? '',
      link: p.link ?? '',
    })),
    awards: (answers.awards ?? []).map((e) => ({
      title: e.title ?? '',
      detail: e.detail ?? '',
      year: e.year ?? '',
    })),
    volunteer: (answers.volunteer ?? []).map((e) => ({
      title: e.title ?? '',
      detail: e.detail ?? '',
      year: e.year ?? '',
    })),
    languages: answers.languages ?? [],
    certifications: answers.certifications ?? [],
  };
}

interface BuilderWizardProps {
  language: string;
  templateId: TemplateId;
  atsMode: boolean;
  isComplete: boolean;
  canUseAI: boolean;
  isGenerating: boolean;
  onLanguageChange: (language: string) => void;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  onGenerate: () => void;
}

export function BuilderWizard({
  language,
  templateId,
  atsMode,
  isComplete,
  canUseAI,
  isGenerating,
  onLanguageChange,
  onTemplateChange,
  onAtsModeChange,
  onGenerate,
}: BuilderWizardProps) {
  const { t } = useTranslation();
  const setResumeBuilder = useSessionStore((s) => s.setResumeBuilder);
  const step = useSessionStore((s) => s.resumeBuilder.wizardStep);

  // Initialize the form ONCE from the current persisted answers. We deliberately
  // do NOT reset the form from a Zustand subscription — RHF is the live editing
  // layer; the slice is the persistence + generation boundary. The blank-default
  // case (Start over) is covered by this component remounting with fresh answers.
  const initialAnswers = useRef(useSessionStore.getState().resumeBuilder.answers);
  const methods = useForm<BuilderForm>({
    defaultValues: toFormValues(initialAnswers.current),
    resolver: zodResolver(builderSchema),
    mode: 'onChange',
  });

  const setStep = (v: number) =>
    setResumeBuilder({ wizardStep: Math.max(0, Math.min(TOTAL_STEPS - 1, v)) });

  // One-way, debounced RHF → Zustand sync so the slice (the synthesis input)
  // stays fresh while the user edits, without per-keystroke store churn.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const sub = methods.watch(() => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        setResumeBuilder({ answers: { ...initialAnswers.current, ...methods.getValues() } });
      }, SYNC_DEBOUNCE_MS);
    });
    return () => {
      if (timer) clearTimeout(timer);
      sub.unsubscribe();
    };
  }, [methods, setResumeBuilder]);

  // Flush the latest values synchronously, then run synthesis (#flush-before-build).
  const onValid = () => {
    setResumeBuilder({ answers: { ...initialAnswers.current, ...methods.getValues() } });
    onGenerate();
  };

  const isLast = step === TOTAL_STEPS - 1;
  const buildDisabled = !isComplete || !canUseAI || !methods.formState.isValid || isGenerating;

  return (
    <motion.div
      key="builder-wizard"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 8 }}
      transition={transition.relaxed}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Top navigation + non-clickable procedure indicator (mirrors GenerateWizard). */}
      <div className="shrink-0 flex items-center justify-between gap-3 px-8 pt-7 pb-2">
        <Button
          onClick={() => setStep(step - 1)}
          disabled={step === 0}
          variant="ghost"
          size="sm"
          className="gap-1.5 text-foreground/50 hover:text-foreground/80 disabled:opacity-0"
        >
          <ChevronLeft size={14} />
          {t('build.wizard.back')}
        </Button>

        <div className="flex flex-col items-center gap-1.5">
          <StepDots currentStep={step} totalSteps={TOTAL_STEPS} className="flex gap-1.5" />
          <span className="text-[10px] uppercase tracking-[0.16em] text-foreground/40">
            {t('build.wizard.stepCounter', { current: step + 1, total: TOTAL_STEPS })}
          </span>
        </div>

        {isLast ? (
          <Button
            onClick={() => void methods.handleSubmit(onValid)()}
            disabled={buildDisabled}
            loading={isGenerating}
            variant="primary"
            size="sm"
            className="gap-1.5 transition-all duration-150 ease-out"
          >
            {!isGenerating && <Sparkles size={13} />}
            {isGenerating ? t('build.wizard.generating') : t('build.wizard.generate')}
          </Button>
        ) : (
          <Button
            onClick={() => setStep(step + 1)}
            variant="primary"
            size="sm"
            className="gap-1.5 transition-all duration-150 ease-out"
          >
            {t('build.wizard.next')}
            <ChevronRight size={14} />
          </Button>
        )}
      </div>

      {/* Step content */}
      <div className="flex-1 overflow-y-auto px-8 pb-6 pt-2" aria-live="polite">
        <FormProvider {...methods}>
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 16 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -16 }}
              transition={transition.normal}
              className="min-h-full"
            >
              <WizardStep
                stepIndex={step}
                totalSteps={TOTAL_STEPS}
                title={t(`build.steps.${step}`)}
                description={t(`build.descriptions.${step}`)}
                align={STEP_ALIGN[step] ?? 'top'}
              >
                {step === 0 && <StepContact />}
                {step === 1 && <StepSummary />}
                {step === 2 && <StepExperience />}
                {step === 3 && <StepEducation />}
                {step === 4 && <StepSkills />}
                {step === 5 && <StepExtras />}
                {step === 6 && (
                  <StepReview
                    language={language}
                    templateId={templateId}
                    atsMode={atsMode}
                    isComplete={isComplete}
                    onLanguageChange={onLanguageChange}
                    onTemplateChange={onTemplateChange}
                    onAtsModeChange={onAtsModeChange}
                  />
                )}
              </WizardStep>
            </motion.div>
          </AnimatePresence>
        </FormProvider>
      </div>
    </motion.div>
  );
}
