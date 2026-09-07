import { ArrowLeft, ArrowRight, Sparkles } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, FloatingIcon, Switch, withDelay } from '@ajh/ui';

import { usePreferencesStore } from '@/store/preferences-store';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Optional onboarding step: index documents automatically instead of waiting for
 * the first match to embed one inline.
 *
 * Asked rather than defaulted because indexing calls the embedding provider, and
 * a cloud provider bills per token — the same reason the Embeddings settings
 * panel carries a cost advisory. Off unless the user turns it on, here and in
 * the stored default, so an install that never sees this screen never starts
 * spending on its own.
 *
 * Placed immediately before the crash-reporting consent step: both are
 * "something runs on your behalf, say yes or no", and grouping them keeps the
 * two spend/privacy decisions together rather than buried among the setup steps.
 */
export function AutoIndexStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const stored = usePreferencesStore((s) => s.autoIndexOnUpload ?? false);
  const setAutoIndexOnUpload = usePreferencesStore((s) => s.setAutoIndexOnUpload);
  // Seeded from the stored value, so stepping BACK and forward again shows the
  // answer already given instead of silently resetting it. Off to begin with:
  // this spends money on a cloud embedding provider, so it is an opt-in, not a
  // default the user has to notice and decline.
  const [checked, setChecked] = useState(stored);

  const advance = () => {
    setAutoIndexOnUpload(checked);
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
        <FloatingIcon icon={Sparkles} size={24} />
      </div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.autoIndex.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.autoIndex.subtitle')}</p>
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
              {t('onboarding.autoIndex.toggleLabel')}
            </div>
            <div className="mt-0.5 text-xs leading-snug text-foreground/40">
              {t('onboarding.autoIndex.what')}
            </div>
            <div className="mt-1.5 text-xs leading-snug text-foreground/40">
              {t('onboarding.autoIndex.cost')}
            </div>
          </div>
          <Switch
            checked={checked}
            onCheckedChange={setChecked}
            aria-label={t('onboarding.autoIndex.toggleLabel')}
          />
        </div>
      </motion.div>

      {/* Same Back/Continue pair every other step renders. The wrapper supplies
          only the Enter/Escape shortcuts, so without these a mouse-only user is
          stranded on the step (#1118) — and Enter does nothing at all while the
          switch has focus, since a focused control owns its own activation. */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        <Button variant="ghost" onClick={onBack} className="flex items-center gap-1.5">
          <ArrowLeft size={13} />
          {t('onboarding.back')}
        </Button>

        <div className="flex-1" />

        <Button variant="primary" onClick={advance} className="flex items-center gap-1.5">
          {t('onboarding.continue')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
