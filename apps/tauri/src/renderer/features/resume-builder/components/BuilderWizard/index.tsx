import { ChevronLeft, ChevronRight, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { Button, StepDots, transition } from '@ajh/ui';

import type { InterviewAnswers, TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useSessionStore } from '@/store/session-store';

import { StepContact } from '../wizard-steps/StepContact';
import { StepEducation } from '../wizard-steps/StepEducation';
import { StepExperience } from '../wizard-steps/StepExperience';
import { StepExtras } from '../wizard-steps/StepExtras';
import { StepReview } from '../wizard-steps/StepReview';
import { StepSkills } from '../wizard-steps/StepSkills';
import { StepSummary } from '../wizard-steps/StepSummary';
import { WizardStep } from '../WizardStep';

const TOTAL_STEPS = 7;

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

interface BuilderWizardProps {
  language: string;
  templateId: TemplateId;
  atsMode: boolean;
  isComplete: boolean;
  canGenerate: boolean;
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
  canGenerate,
  isGenerating,
  onLanguageChange,
  onTemplateChange,
  onAtsModeChange,
  onGenerate,
}: BuilderWizardProps) {
  const { t } = useTranslation();
  const resumeBuilder = useSessionStore((s) => s.resumeBuilder);
  const setResumeBuilder = useSessionStore((s) => s.setResumeBuilder);

  const { answers, wizardStep: step } = resumeBuilder;
  const setStep = (v: number) =>
    setResumeBuilder({ wizardStep: Math.max(0, Math.min(TOTAL_STEPS - 1, v)) });
  const update = (patch: Partial<InterviewAnswers>) =>
    setResumeBuilder({ answers: { ...answers, ...patch } });

  const isLast = step === TOTAL_STEPS - 1;

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
            onClick={onGenerate}
            disabled={!canGenerate || isGenerating}
            loading={isGenerating}
            variant="glass"
            size="sm"
            className="gap-1.5 transition-all duration-150 ease-out"
          >
            {!isGenerating && <Sparkles size={13} />}
            {isGenerating ? t('build.wizard.generating') : t('build.wizard.generate')}
          </Button>
        ) : (
          <Button
            onClick={() => setStep(step + 1)}
            variant="glass"
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
              {step === 0 && <StepContact answers={answers} update={update} />}
              {step === 1 && <StepSummary answers={answers} update={update} />}
              {step === 2 && <StepExperience answers={answers} update={update} />}
              {step === 3 && <StepEducation answers={answers} update={update} />}
              {step === 4 && <StepSkills answers={answers} update={update} />}
              {step === 5 && <StepExtras answers={answers} update={update} />}
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
      </div>
    </motion.div>
  );
}
