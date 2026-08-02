import { ArrowLeft, ArrowRight, LifeBuoy } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, FloatingIcon, Switch, withDelay } from '@ajh/ui';

import { useCrashReporting, useSetCrashReporting } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Crash-reporting consent. On by default — but nothing is transmitted until
 * this step has actually been shown, which is what advancing from here records
 * (`consentShown: true`).
 *
 * That ordering is the whole point of the step: a default the user never saw is
 * not a choice, and the gap between a consent screen and what the code really
 * does is where privacy claims break. A user who never finishes onboarding
 * therefore never reports anything, which is the safe direction to fail.
 */
export function CrashReportingStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const { data } = useCrashReporting();
  const setCrashReporting = useSetCrashReporting();
  // Local mirror so the switch stays responsive; the persisted value is written
  // once on advance rather than on every flick.
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const checked = enabled ?? data?.enabled ?? true;

  const advance = () => {
    // Fire-and-forget: a failed write leaves consentShown false, which means
    // "not asked" and therefore no transmission — the safe direction.
    setCrashReporting.mutate({ enabled: checked, consentShown: true });
    onNext();
  };

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={advance}
      canAdvance
    >
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={LifeBuoy} size={24} />
      </div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.crashReporting.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.crashReporting.subtitle')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-6"
      >
        <div className="flex items-start gap-4 rounded-xl border border-foreground/10 px-4 py-3.5">
          <div className="min-w-0 flex-1">
            <div className="text-sm font-semibold text-foreground/90">
              {t('onboarding.crashReporting.toggleLabel')}
            </div>
            <div className="mt-0.5 text-xs leading-snug text-foreground/40">
              {t('onboarding.crashReporting.sends')}
            </div>
            <div className="mt-1.5 text-xs leading-snug text-foreground/40">
              {t('onboarding.crashReporting.neverSends')}
            </div>
          </div>
          <Switch
            checked={checked}
            onCheckedChange={setEnabled}
            aria-label={t('onboarding.crashReporting.toggleLabel')}
          />
        </div>
        <p className="mt-2 text-[10px] text-foreground/30">
          {t('onboarding.crashReporting.changeLater')}
        </p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        <Button variant="ghost" onClick={onBack} className="flex items-center gap-1.5">
          <ArrowLeft size={13} />
          {t('onboarding.crashReporting.back')}
        </Button>

        <div className="flex-1" />

        <Button variant="primary" onClick={advance} className="flex items-center gap-1.5">
          {t('onboarding.crashReporting.next')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
