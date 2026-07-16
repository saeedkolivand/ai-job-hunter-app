import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { transition } from '@ajh/ui';

import { useActiveConfig } from '@/services';
import { useOnboardingCompleted, usePreferencesStore } from '@/store/preferences-store';

import { SpotlightTour } from '../SpotlightTour';
import { ONBOARDING_STEPS } from '../steps-config';

export function OnboardingWizard() {
  const onboardingCompleted = useOnboardingCompleted();
  const setOnboardingComplete = usePreferencesStore((s) => s.setOnboardingComplete);
  const setSidebarCollapsed = usePreferencesStore((s) => s.setSidebarCollapsed);
  // Active provider is backend-owned (task #16); the research step is Ollama-only.
  const activeProvider = useActiveConfig().data?.activeProvider;
  const [stepIndex, setStepIndex] = useState(0);
  const [direction, setDirection] = useState(1);
  const [showTour, setShowTour] = useState(false);
  // Snapshot the user's sidebar preference from right before the tour forces
  // it open, so it can be restored on finish instead of silently overwritten.
  const sidebarWasCollapsedRef = useRef(false);

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
      // The tour spotlights sidebar nav items — a collapsed sidebar unmounts
      // them, so force it open before the tour measures its anchors. Snapshot
      // the prior value first so it can be restored once the tour ends.
      sidebarWasCollapsedRef.current = usePreferencesStore.getState().sidebarCollapsed ?? false;
      setSidebarCollapsed(false);
      setShowTour(true);
    }
  };

  // Restore the user's original sidebar preference (if it was collapsed) once
  // the tour ends, then mark onboarding complete — runs on both "finish" and
  // "skip" since SpotlightTour's onFinish covers both.
  const finishTour = () => {
    if (sidebarWasCollapsedRef.current) setSidebarCollapsed(true);
    setOnboardingComplete();
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
        {showTour && <SpotlightTour key="tour" onFinish={finishTour} />}
      </AnimatePresence>
    </div>
  );
}
