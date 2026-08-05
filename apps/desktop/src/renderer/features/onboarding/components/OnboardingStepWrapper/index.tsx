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
  // Global keyboard nav for the onboarding flow: Enter advances, Escape goes
  // back — but only when focus isn't on something that already owns Enter
  // itself. A window listener keeps the interaction off the non-interactive
  // step container (jsx-a11y).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const activeElement = document.activeElement;
      // Typing surfaces, and anything that gets its own browser-synthesized
      // `click` on Enter (button incl. role="button", link, select) — Enter
      // there means "activate what I'm focused on" (pick this language tile,
      // toggle this theme swatch, open this dropdown), not "advance the
      // step". Letting the global shortcut ALSO fire in that case either
      // double-advances when the focused button's own click already calls
      // onNext (Continue — #939), or silently steals Enter from a control
      // whose click does something else entirely. The global "advance"
      // shortcut is reserved for focus that isn't on an interactive control
      // at all (e.g. the step container itself).
      const activeElementHandlesOwnActivation =
        activeElement instanceof HTMLInputElement ||
        activeElement instanceof HTMLTextAreaElement ||
        activeElement instanceof HTMLButtonElement ||
        activeElement instanceof HTMLAnchorElement ||
        activeElement instanceof HTMLSelectElement ||
        (activeElement instanceof HTMLElement && activeElement.getAttribute('role') === 'button');

      if (e.key === 'Enter' && canAdvance && onNext && !activeElementHandlesOwnActivation) {
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
        className="@container rounded-2xl border border-[var(--border-clear)] p-8 onboarding-glass-modal"
      >
        {children}
        {showStepDots && <StepDots currentStep={stepIndex} totalSteps={totalSteps} />}
      </div>
    </motion.div>
  );
}
