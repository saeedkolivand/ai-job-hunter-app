/**
 * Mock factories for the OnboardingWizard tests. `vi.mock` is hoisted per test
 * file, so each step is wired in with
 * `vi.mock('../steps/WelcomeStep', async () => ({ WelcomeStep: (await import('./test-mocks')).stepStub('stepWelcome') }))`.
 *
 * Each stub renders a root element carrying its step's data-testid so tests can
 * assert which step is visible, plus buttons that forward onNext/onBack. Button
 * from @ajh/ui is used (raw <button> is banned in renderer files).
 */
import { TEST_IDS } from '@ajh/test-ids';
import { Button } from '@ajh/ui';

interface StepStubProps {
  onNext: () => void;
  onBack?: () => void;
  stepIndex: number;
  totalSteps: number;
}

/** A lightweight step exposing its props (stepIndex, totalSteps, onNext, onBack) via the DOM. */
export function stepStub(id: keyof typeof TEST_IDS.onboarding) {
  return function StepStub({ onNext, onBack, stepIndex, totalSteps }: StepStubProps) {
    return (
      <div
        data-testid={TEST_IDS.onboarding[id]}
        data-step-index={stepIndex}
        data-total-steps={totalSteps}
      >
        <Button onClick={onNext}>next</Button>
        {onBack && <Button onClick={onBack}>back</Button>}
      </div>
    );
  };
}

/** SpotlightTour reduced to a marker plus a finish button. */
export function tourStub({ onFinish }: { onFinish: () => void }) {
  return (
    <div data-testid={TEST_IDS.onboarding.tour}>
      <Button onClick={onFinish}>finish-tour</Button>
    </div>
  );
}
