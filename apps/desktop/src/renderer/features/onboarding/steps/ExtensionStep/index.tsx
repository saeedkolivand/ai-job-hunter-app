import { ArrowLeft, ArrowRight, Puzzle } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, FloatingIcon, withDelay } from '@ajh/ui';

import { useOpenExternal } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

const CHROME_WEB_STORE_URL =
  'https://chromewebstore.google.com/detail/oaoekkgkhmgdfnpmfkpphgiikliaicll';
const FIREFOX_AMO_URL =
  'https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter-job-importer/';

interface Props {
  onBack?: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Optional onboarding step: surface the browser extension so users can import
 * jobs straight from any board. Chrome and Firefox are both live. The step is
 * never required to advance.
 */
export function ExtensionStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance
    >
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={Puzzle} size={24} />
      </div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.extension.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.extension.subtitle')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-6 space-y-2"
      >
        <Button
          variant="primary"
          onClick={() => void openExternal.mutateAsync(CHROME_WEB_STORE_URL)}
          className="w-full justify-center"
        >
          {t('onboarding.extension.addToChrome')}
        </Button>
        <Button
          variant="default"
          onClick={() => void openExternal.mutateAsync(FIREFOX_AMO_URL)}
          className="w-full justify-center"
        >
          {t('onboarding.extension.addToFirefox')}
        </Button>
        <p className="text-[10px] text-foreground/30">{t('onboarding.extension.pairHint')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        {onBack && (
          <Button variant="ghost" onClick={onBack} className="flex items-center gap-1.5">
            <ArrowLeft size={13} />
            {t('onboarding.extension.back')}
          </Button>
        )}

        <div className="flex-1" />

        <Button variant="primary" onClick={onNext} className="flex items-center gap-1.5">
          {t('onboarding.extension.next')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
