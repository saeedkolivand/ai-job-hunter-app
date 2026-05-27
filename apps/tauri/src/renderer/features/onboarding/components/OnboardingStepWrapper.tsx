import { motion } from 'motion/react';
import { useEffect } from 'react';

import { StepDots } from '@ajh/ui';

import { transition } from '@ajh/ui';

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
  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Don't handle Enter if focused on an input/textarea
    const activeElement = document.activeElement;
    const isInputFocused =
      activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement;

    if (e.key === 'Enter' && canAdvance && onNext && !isInputFocused) {
      onNext();
    } else if (e.key === 'Escape' && onBack) {
      onBack();
    }
  };

  // Auto-focus the wrapper when step changes
  useEffect(() => {
    const wrapper = document.querySelector('[data-onboarding-wrapper]');
    if (wrapper instanceof HTMLElement) {
      wrapper.focus();
    }
  }, [stepIndex]);

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
        className="rounded-2xl border border-white/[0.08] p-8 onboarding-glass-modal"
        onKeyDown={handleKeyDown}
        tabIndex={0}
      >
        {children}
        {showStepDots && <StepDots currentStep={stepIndex} totalSteps={totalSteps} />}
      </div>
    </motion.div>
  );
}
