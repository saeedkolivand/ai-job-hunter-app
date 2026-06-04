import { Check, ChevronLeft, ChevronRight, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { Button, cn, transition } from '@ajh/ui';

import { type GenerationMode, isTwoColumnTemplate, type TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useSessionStore } from '@/store/session-store';

import { StepFineTune } from '../wizard-steps/StepFineTune';
import { StepTarget } from '../wizard-steps/StepTarget';
import { StepTemplate } from '../wizard-steps/StepTemplate';

const TOTAL_STEPS = 3;

interface GenerateWizardProps {
  mode: GenerationMode;
  target: 'resume' | 'cover' | 'both';
  templateId: TemplateId;
  atsMode: boolean;
  locale: string;
  researchCompany: boolean;
  isGenerating: boolean;
  onModeChange: (mode: GenerationMode) => void;
  onTargetChange: (t: 'resume' | 'cover' | 'both') => void;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  onLocaleChange: (locale: string) => void;
  onResearchCompanyChange: (v: boolean) => void;
  onGenerate: () => void;
}

export function GenerateWizard({
  mode,
  target,
  templateId,
  atsMode,
  locale,
  researchCompany,
  isGenerating,
  onModeChange,
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
    if (!isTwoColumnTemplate(id)) {
      onAtsModeChange(false);
    }
  };

  return (
    <motion.div
      key="wizard"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 8 }}
      transition={transition.relaxed}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Step indicators */}
      <div className="shrink-0 flex items-center gap-0 px-8 pt-8 pb-4">
        {Array.from({ length: TOTAL_STEPS }, (_, i) => (
          <div
            key={i}
            className={cn(
              'flex flex-1 items-center justify-center gap-1.5 py-2 text-[11px] font-medium transition-colors',
              i === step
                ? 'text-brand-soft'
                : i < step
                  ? 'text-foreground/40'
                  : 'text-foreground/25'
            )}
          >
            {i < step ? (
              <Check size={10} className="text-emerald-400" />
            ) : (
              <span className="h-4 w-4 rounded-full border border-current flex items-center justify-center text-[9px]">
                {i + 1}
              </span>
            )}
            {t(`aiGenerate.wizard.steps.${i}`)}
          </div>
        ))}
      </div>

      {/* Step content */}
      <div className="flex-1 overflow-y-auto px-8 pb-4" aria-live="polite">
        <AnimatePresence mode="wait">
          <motion.div
            key={step}
            initial={{ opacity: 0, x: 16 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -16 }}
            transition={transition.normal}
          >
            {step === 0 && <StepTarget target={target} onTargetChange={onTargetChange} />}
            {step === 1 && (
              <StepTemplate
                templateId={templateId}
                atsMode={atsMode}
                onTemplateChange={handleTemplateChange}
                onAtsModeChange={onAtsModeChange}
              />
            )}
            {step === 2 && (
              <StepFineTune
                mode={mode}
                target={target}
                locale={locale}
                researchCompany={researchCompany}
                onModeChange={onModeChange}
                onLocaleChange={onLocaleChange}
                onResearchCompanyChange={onResearchCompanyChange}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>

      {/* Footer — Back / Next / Generate */}
      <div className="shrink-0 flex items-center justify-between border-t border-white/[0.07] px-8 py-4">
        <Button
          onClick={() => setStep(step - 1)}
          disabled={step === 0}
          className="flex items-center gap-1.5 text-xs text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent disabled:opacity-0"
        >
          <ChevronLeft size={13} />
          {t('aiGenerate.wizard.back')}
        </Button>

        {step < TOTAL_STEPS - 1 ? (
          <Button
            variant="glass"
            size="sm"
            onClick={() => setStep(step + 1)}
            className="flex items-center gap-1.5 transition-all duration-150 ease-out"
          >
            {t('aiGenerate.wizard.next')}
            <ChevronRight size={13} />
          </Button>
        ) : (
          <Button
            variant="glass"
            size="sm"
            onClick={onGenerate}
            disabled={isGenerating}
            loading={isGenerating}
            className="flex items-center gap-1.5 transition-all duration-150 ease-out"
          >
            {!isGenerating && <Sparkles size={13} />}
            {isGenerating ? t('aiGenerate.generating') : t('aiGenerate.wizard.generate')}
          </Button>
        )}
      </div>
    </motion.div>
  );
}
