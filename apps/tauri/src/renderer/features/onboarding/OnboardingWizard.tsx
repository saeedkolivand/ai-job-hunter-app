import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { transition } from '@/lib/motion';
import { useOnboardingCompleted, usePreferencesStore } from '@/store/preferences-store';

import { SpotlightTour } from './SpotlightTour';
import { OllamaStep } from './steps/OllamaStep';
import { PrefsStep } from './steps/PrefsStep';
import { ResumeStep } from './steps/ResumeStep';
import { WelcomeStep } from './steps/WelcomeStep';

type Step = 'welcome' | 'prefs' | 'resume' | 'ollama' | 'tour';

export function OnboardingWizard() {
  const onboardingCompleted = useOnboardingCompleted();
  const setOnboardingComplete = usePreferencesStore((s) => s.setOnboardingComplete);
  const [step, setStep] = useState<Step>('welcome');
  const [direction, setDirection] = useState(1);

  const goNext = (next: Step) => {
    setDirection(1);
    setStep(next);
  };

  const goBack = (prev: Step) => {
    setDirection(-1);
    setStep(prev);
  };

  if (onboardingCompleted) return null;

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center overflow-hidden">
      {/* Backdrop — hidden during tour so the app UI is visible */}
      <AnimatePresence>
        {step !== 'tour' && (
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
        {step === 'welcome' && (
          <WelcomeStep key="welcome" direction={direction} onNext={() => goNext('prefs')} />
        )}
        {step === 'prefs' && (
          <PrefsStep
            key="prefs"
            direction={direction}
            onBack={() => goBack('welcome')}
            onNext={() => goNext('resume')}
          />
        )}
        {step === 'resume' && (
          <ResumeStep
            key="resume"
            direction={direction}
            onBack={() => goBack('prefs')}
            onNext={() => goNext('ollama')}
          />
        )}
        {step === 'ollama' && (
          <OllamaStep
            key="ollama"
            direction={direction}
            onBack={() => goBack('resume')}
            onNext={() => goNext('tour')}
          />
        )}
        {step === 'tour' && <SpotlightTour key="tour" onFinish={setOnboardingComplete} />}
      </AnimatePresence>
    </div>
  );
}
