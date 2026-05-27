import { useTranslation } from '@/lib/i18n';
import { useCheckBrowser } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';
import { BrowserDetectedState } from '../browser/BrowserDetectedState';
import { BrowserErrorState } from '../browser/BrowserErrorState';
import { BrowserLoadingState } from '../browser/BrowserLoadingState';
import { BrowserNotDetectedState } from '../browser/BrowserNotDetectedState';

interface BrowserStepProps {
  direction: number;
  onBack: () => void;
  onNext: () => void;
  stepIndex: number;
  totalSteps: number;
}

export function BrowserStep({
  direction,
  onBack,
  onNext,
  stepIndex,
  totalSteps,
}: BrowserStepProps) {
  const { t } = useTranslation();
  const { data: browserCheck, isLoading, error } = useCheckBrowser();

  const isDetected = browserCheck?.detected === true;
  const browserPath = browserCheck?.path || '';
  const hasError = !!error;

  const showLoading = isLoading && !browserCheck;
  const showError = hasError && !browserCheck;
  const showDetected = isDetected && !showError;

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance={isDetected || !showLoading}
    >
      {/* Loading state */}
      {showLoading && <BrowserLoadingState message={t('onboarding.browser.checking')} />}

      {/* Error state */}
      {showError && <BrowserErrorState onBack={onBack} onNext={onNext} />}

      {/* Detected state */}
      {showDetected && (
        <BrowserDetectedState browserPath={browserPath} onBack={onBack} onNext={onNext} />
      )}

      {/* Not detected state */}
      {!showDetected && !showError && !showLoading && (
        <BrowserNotDetectedState onBack={onBack} onNext={onNext} />
      )}
    </OnboardingStepWrapper>
  );
}
