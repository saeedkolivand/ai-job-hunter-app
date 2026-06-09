import { ChevronLeft, ChevronRight, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { Button, StepDots, transition } from '@ajh/ui';

import {
  type EmphasisId,
  type GenerationMode,
  isTwoColumnTemplate,
  type TemplateId,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useSessionStore } from '@/store/session-store';

import { StepFineTune } from '../wizard-steps/StepFineTune';
import { StepTarget } from '../wizard-steps/StepTarget';
import { StepTemplate } from '../wizard-steps/StepTemplate';

const TOTAL_STEPS = 3;

interface GenerateWizardProps {
  mode: GenerationMode;
  emphasis: EmphasisId[];
  target: 'resume' | 'cover' | 'both';
  templateId: TemplateId;
  atsMode: boolean;
  locale: string;
  researchCompany: boolean;
  isGenerating: boolean;
  onModeChange: (mode: GenerationMode) => void;
  onEmphasisChange: (ids: EmphasisId[]) => void;
  onTargetChange: (t: 'resume' | 'cover' | 'both') => void;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  onLocaleChange: (locale: string) => void;
  onResearchCompanyChange: (v: boolean) => void;
  onGenerate: () => void;
}

export function GenerateWizard({
  mode,
  emphasis,
  target,
  templateId,
  atsMode,
  locale,
  researchCompany,
  isGenerating,
  onModeChange,
  onEmphasisChange,
  onTargetChange,
  onTemplateChange,
  onAtsModeChange,
  onLocaleChange,
  onResearchCompanyChange,
  onGenerate,
}: GenerateWizardProps) {
  const { t } = useTranslation();
  const { aiGenerate, setAIGenerate } = useSessionStore();
  const step = aiGenerate.wizardStep;
  const setStep = (v: number) =>
    setAIGenerate({ wizardStep: Math.max(0, Math.min(TOTAL_STEPS - 1, v)) });

  const handleTemplateChange = (id: TemplateId) => {
    onTemplateChange(id);
    // Cover letters have no ATS toggle, so never touch atsMode there.
    if (target !== 'cover' && !isTwoColumnTemplate(id)) {
      onAtsModeChange(false);
    }
  };

  // #10 — picking a target on step 1 doubles as "Next": select and advance in one
  // click, so there is no separate Next on the first step.
  const handleTargetChange = (tgt: 'resume' | 'cover' | 'both') => {
    onTargetChange(tgt);
    setStep(1);
  };

  const isLast = step === TOTAL_STEPS - 1;

  return (
    <motion.div
      key="wizard"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 8 }}
      transition={transition.relaxed}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Top navigation (#14) + procedure indicator (#11, non-clickable) */}
      <div className="shrink-0 flex items-center justify-between gap-3 px-8 pt-7 pb-2">
        <Button
          onClick={() => setStep(step - 1)}
          disabled={step === 0}
          variant="ghost"
          size="sm"
          className="gap-1.5 text-foreground/50 hover:text-foreground/80 disabled:opacity-0"
        >
          <ChevronLeft size={14} />
          {t('aiGenerate.wizard.back')}
        </Button>

        <div className="flex flex-col items-center gap-1.5">
          <StepDots currentStep={step} totalSteps={TOTAL_STEPS} className="flex gap-1.5" />
          <span className="text-[10px] uppercase tracking-[0.16em] text-foreground/40">
            {t('aiGenerate.wizard.stepCounter', { current: step + 1, total: TOTAL_STEPS })}
          </span>
        </div>

        {/* Step 0 advances by selecting a target (#10), so its Next is hidden. */}
        {step === 0 ? (
          <span className="w-[64px]" aria-hidden />
        ) : isLast ? (
          <Button
            onClick={onGenerate}
            disabled={isGenerating}
            loading={isGenerating}
            variant="glass"
            size="sm"
            className="gap-1.5 transition-all duration-150 ease-out"
          >
            {!isGenerating && <Sparkles size={13} />}
            {isGenerating ? t('aiGenerate.generating') : t('aiGenerate.wizard.generate')}
          </Button>
        ) : (
          <Button
            onClick={() => setStep(step + 1)}
            variant="glass"
            size="sm"
            className="gap-1.5 transition-all duration-150 ease-out"
          >
            {t('aiGenerate.wizard.next')}
            <ChevronRight size={14} />
          </Button>
        )}
      </div>

      {/* Step content */}
      <div className="flex-1 overflow-y-auto px-8 pb-6 pt-2" aria-live="polite">
        {/* Descriptive step title + purpose (#17) — centralized so each step body
            renders only its controls. */}
        <div className="mb-4">
          <h2 className="text-sm font-semibold text-foreground/85">
            {target === 'cover' && step === 1
              ? t('aiGenerate.wizard.coverStyle.title')
              : t(`aiGenerate.wizard.steps.${step}`)}
          </h2>
          <p className="mt-0.5 text-xs text-foreground/40">
            {target === 'cover' && step === 1
              ? t('aiGenerate.wizard.coverStyle.desc')
              : t(`aiGenerate.wizard.descriptions.${step}`)}
          </p>
        </div>

        <AnimatePresence mode="wait">
          <motion.div
            key={step}
            initial={{ opacity: 0, x: 16 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -16 }}
            transition={transition.normal}
          >
            {step === 0 && <StepTarget target={target} onTargetChange={handleTargetChange} />}
            {step === 1 && (
              <StepTemplate
                templateId={templateId}
                atsMode={atsMode}
                onTemplateChange={handleTemplateChange}
                onAtsModeChange={onAtsModeChange}
                target={target}
              />
            )}
            {step === 2 && (
              <StepFineTune
                mode={mode}
                emphasis={emphasis}
                target={target}
                locale={locale}
                researchCompany={researchCompany}
                onModeChange={onModeChange}
                onEmphasisChange={onEmphasisChange}
                onLocaleChange={onLocaleChange}
                onResearchCompanyChange={onResearchCompanyChange}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
