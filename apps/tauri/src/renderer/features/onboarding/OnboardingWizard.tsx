import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useState } from 'react';

import { transition } from '@/lib/motion';
import { useOnboardingCompleted, usePreferencesStore } from '@/store/preferences-store';

import { SpotlightTour } from './SpotlightTour';
import { AISelectionStep } from './steps/AISelectionStep';
import { BrowserStep } from './steps/BrowserStep';
import { ResumeStep } from './steps/ResumeStep';
import { WelcomeStep } from './steps/WelcomeStep';
import { ONBOARDING_STEPS, TOTAL_STEPS } from './steps-config';

export function OnboardingWizard() {
  const onboardingCompleted = useOnboardingCompleted();
  const setOnboardingComplete = usePreferencesStore((s) => s.setOnboardingComplete);
  const [stepIndex, setStepIndex] = useState(0);
  const [direction, setDirection] = useState(1);
  const [showTour, setShowTour] = useState(false);

  // Reset wizard state when onboarding is restarted (e.g., after app reset)
  useEffect(() => {
    if (!onboardingCompleted) {
      setStepIndex(0);
      setDirection(1);
      setShowTour(false);
    }
  }, [onboardingCompleted]);

  const currentStep = ONBOARDING_STEPS[stepIndex];
  const stepId = showTour ? 'tour' : (currentStep?.id ?? 'welcome');

  const goNext = () => {
    if (stepIndex < ONBOARDING_STEPS.length - 1) {
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
        {stepId === 'welcome' && (
          <WelcomeStep
            key="welcome"
            direction={direction}
            stepIndex={0}
            totalSteps={TOTAL_STEPS}
            onNext={goNext}
          />
        )}
        {stepId === 'resume' && (
          <ResumeStep
            key="resume"
            direction={direction}
            stepIndex={1}
            totalSteps={TOTAL_STEPS}
            onBack={goBack}
            onNext={goNext}
          />
        )}
        {stepId === 'ai' && (
          <AISelectionStep
            key="ai"
            direction={direction}
            stepIndex={2}
            totalSteps={TOTAL_STEPS}
            onBack={goBack}
            onNext={goNext}
          />
        )}
        {stepId === 'browser' && (
          <BrowserStep
            key="browser"
            direction={direction}
            stepIndex={3}
            totalSteps={TOTAL_STEPS}
            onBack={goBack}
            onNext={goNext}
          />
        )}
        {showTour && <SpotlightTour key="tour" onFinish={setOnboardingComplete} />}
      </AnimatePresence>
    </div>
  );
}
