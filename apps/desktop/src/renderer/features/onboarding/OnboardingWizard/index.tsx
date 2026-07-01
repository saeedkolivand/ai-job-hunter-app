import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useState } from 'react';

import { transition } from '@ajh/ui';

import { useOnboardingCompleted, usePreferencesStore } from '@/store/preferences-store';

import { SpotlightTour } from '../SpotlightTour';
import { ONBOARDING_STEPS } from '../steps-config';

export function OnboardingWizard() {
  const onboardingCompleted = useOnboardingCompleted();
  const setOnboardingComplete = usePreferencesStore((s) => s.setOnboardingComplete);
  const activeProvider = usePreferencesStore((s) => s.aiProviderConfig?.activeProvider);
  const [stepIndex, setStepIndex] = useState(0);
  const [direction, setDirection] = useState(1);
  const [showTour, setShowTour] = useState(false);

  // Research is only relevant for local Ollama (other providers search with
  // their own key). `extension`/`appearance` are unconditional.
  const steps = ONBOARDING_STEPS.filter((s) => s.id !== 'research' || activeProvider === 'ollama');

  // Reset wizard state when onboarding is restarted (e.g., after app reset)
  useEffect(() => {
    if (!onboardingCompleted) {
      setStepIndex(0);
      setDirection(1);
      setShowTour(false);
    }
  }, [onboardingCompleted]);

  // A provider flip can shorten the step list — clamp the index so it can't
  // strand past the end of the array.
  useEffect(() => {
    setStepIndex((i) => Math.min(i, steps.length - 1));
  }, [steps.length]);

  const goNext = () => {
    if (stepIndex < steps.length - 1) {
      setDirection(1);
      setStepIndex((prev) => prev + 1);
    } else {
      setDirection(1);
      setShowTour(true);
    }
  };

  const goBack = () => {
    if (stepIndex > 0) {
      setDirection(-1);
      setStepIndex((prev) => prev - 1);
    }
  };

  if (onboardingCompleted) return null;

  const currentStep = steps[stepIndex] ?? steps[0];
  if (!currentStep) return null;
  const Current = currentStep.component;

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center overflow-hidden">
      {/* Backdrop — hidden during tour so the app UI is visible */}
      <AnimatePresence>
        {!showTour && (
          <motion.div
            key="backdrop"
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={transition.normal}
          />
        )}
      </AnimatePresence>

      <AnimatePresence mode="wait" custom={direction}>
        {!showTour && (
          <Current
            key={currentStep.id}
            direction={direction}
            stepIndex={stepIndex}
            totalSteps={steps.length}
            onNext={goNext}
            onBack={goBack}
          />
        )}
        {showTour && <SpotlightTour key="tour" onFinish={setOnboardingComplete} />}
      </AnimatePresence>
    </div>
  );
}
