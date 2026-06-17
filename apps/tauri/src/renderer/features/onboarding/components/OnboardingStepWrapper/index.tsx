import { motion } from 'motion/react';
import { useEffect } from 'react';

import { StepDots, transition } from '@ajh/ui';

interface OnboardingStepWrapperProps {
  children: React.ReactNode;
  direction: number;
  stepIndex: number;
  totalSteps: number;
  onNext?: () => void;
  onBack?: () => void;
  canAdvance?: boolean;
  showStepDots?: boolean;
  className?: string;
}

export function OnboardingStepWrapper({
  children,
  direction,
  stepIndex,
  totalSteps,
  onNext,
  onBack,
  canAdvance = true,
  showStepDots = true,
  className = '',
}: OnboardingStepWrapperProps) {
  // Global keyboard nav for the onboarding flow: Enter advances (unless typing
  // in an input/textarea), Escape goes back. A window listener keeps the
  // interaction off the non-interactive step container (jsx-a11y).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const activeElement = document.activeElement;
      const isInputFocused =
        activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement;

      if (e.key === 'Enter' && canAdvance && onNext && !isInputFocused) {
        onNext();
      } else if (e.key === 'Escape' && onBack) {
        onBack();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [canAdvance, onNext, onBack]);

  return (
    <motion.div
      custom={direction}
      variants={{
        initial: (dir: number) => ({ opacity: 0, x: dir * 60 }),
        animate: { opacity: 1, x: 0 },
        exit: (dir: number) => ({ opacity: 0, x: dir * -60 }),
      }}
      initial="initial"
      animate="animate"
      exit="exit"
      transition={transition.modal}
      className={`relative z-10 w-full max-w-lg mx-4 ${className}`}
    >
      <div
        data-onboarding-wrapper
        className="@container rounded-2xl border border-white/[0.08] p-8 onboarding-glass-modal"
      >
        {children}
        {showStepDots && <StepDots currentStep={stepIndex} totalSteps={totalSteps} />}
      </div>
    </motion.div>
  );
}
