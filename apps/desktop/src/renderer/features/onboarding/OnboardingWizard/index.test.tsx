/**
 * OnboardingWizard — step filter, navigation, clamp, and completion-gate tests.
 *
 * Strategy:
 *  - All step components are stubbed to lightweight buttons that expose their
 *    props (stepIndex, totalSteps, onNext, onBack) via data-testid attributes.
 *    This keeps the filter/clamp/nav logic under test without dragging in
 *    every step's service dependencies.
 *  - SpotlightTour is stubbed to a single marker element so we can assert the
 *    wizard transitions to the tour on last-step onNext.
 *  - usePreferencesStore.setState is used to seed provider and completed state.
 *    The store is reset in beforeEach so tests don't bleed into each other.
 *  - @ajh/translations returns keys as-is (key-passthrough pattern).
 *  - motion/react is not mocked — AnimatePresence renders synchronously in
 *    jsdom with no layout side-effects that need suppressing.
 *
 * noUncheckedIndexedAccess: all array accesses are guarded with null-checks.
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import { Button } from '@ajh/ui';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Step component stubs ──────────────────────────────────────────────────────
// Each stub renders a root element carrying data-testid so tests can assert
// which step is visible, plus buttons that forward onNext/onBack. The step id
// literal is embedded in data-testid to distinguish stubs from one another.
// Button from @ajh/ui is used (raw <button> is banned in renderer files).

vi.mock('../steps/WelcomeStep', () => ({
  WelcomeStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepWelcome}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/ResumeStep', () => ({
  ResumeStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepResume}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/AISelectionStep', () => ({
  AISelectionStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepAi}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/ResearchStep', () => ({
  ResearchStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepResearch}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/BrowserStep', () => ({
  BrowserStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepBrowser}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/AdzunaKeyStep', () => ({
  AdzunaKeyStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepAdzunaKey}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/ExtensionStep', () => ({
  ExtensionStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepExtension}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

vi.mock('../steps/AppearanceStep', () => ({
  AppearanceStep: ({
    onNext,
    onBack,
    stepIndex,
    totalSteps,
  }: {
    onNext: () => void;
    onBack?: () => void;
    stepIndex: number;
    totalSteps: number;
  }) => (
    <div
      data-testid={TEST_IDS.onboarding.stepAppearance}
      data-step-index={stepIndex}
      data-total-steps={totalSteps}
    >
      <Button onClick={onNext}>next</Button>
      {onBack && <Button onClick={onBack}>back</Button>}
    </div>
  ),
}));

// ── SpotlightTour stub ────────────────────────────────────────────────────────

vi.mock('../SpotlightTour', () => ({
  SpotlightTour: ({ onFinish }: { onFinish: () => void }) => (
    <div data-testid={TEST_IDS.onboarding.tour}>
      <Button onClick={onFinish}>finish-tour</Button>
    </div>
  ),
}));

// ── component under test (imported AFTER mocks) ───────────────────────────────

import { OnboardingWizard } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderWizard() {
  const client = createMockClient();
  return render(<OnboardingWizard />, { wrapper: withProviders(client) });
}

/** Return the data-total-steps attribute of the currently visible step. */
function totalStepsOf(el: HTMLElement): number {
  return Number(el.getAttribute('data-total-steps'));
}

function stepIndexOf(el: HTMLElement): number {
  return Number(el.getAttribute('data-step-index'));
}

/** Click the "next" button inside a step stub element. */
async function clickNext(user: ReturnType<typeof userEvent.setup>, stepEl: HTMLElement) {
  await user.click(within(stepEl).getByRole('button', { name: 'next' }));
}

/** Click the "back" button inside a step stub element. */
async function clickBack(user: ReturnType<typeof userEvent.setup>, stepEl: HTMLElement) {
  await user.click(within(stepEl).getByRole('button', { name: 'back' }));
}

// ── store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  act(() => {
    usePreferencesStore.setState({
      onboardingCompleted: false,
      aiProviderConfig: undefined,
    });
  });
});

// ── tests ─────────────────────────────────────────────────────────────────────

describe('OnboardingWizard — step filter', () => {
  it('includes research step (8 total) when activeProvider is ollama', () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'ollama', providers: {} },
    });
    renderWizard();

    const welcome = screen.getByTestId(TEST_IDS.onboarding.stepWelcome);
    expect(totalStepsOf(welcome)).toBe(8);
  });

  it('excludes research step (7 total) when activeProvider is openai', () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
    });
    renderWizard();

    const welcome = screen.getByTestId(TEST_IDS.onboarding.stepWelcome);
    expect(totalStepsOf(welcome)).toBe(7);
  });

  it('excludes research step (7 total) when activeProvider is undefined', () => {
    usePreferencesStore.setState({ aiProviderConfig: undefined });
    renderWizard();

    const welcome = screen.getByTestId(TEST_IDS.onboarding.stepWelcome);
    expect(totalStepsOf(welcome)).toBe(7);
  });

  it('research stub is present in the DOM when ollama is active after navigating to it', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'ollama', providers: {} },
    });
    const user = userEvent.setup();
    renderWizard();

    // welcome → resume → ai → research (index 3)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));

    expect(screen.getByTestId(TEST_IDS.onboarding.stepResearch)).toBeInTheDocument();
  });

  it('research stub never appears when activeProvider is openai', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
    });
    const user = userEvent.setup();
    renderWizard();

    // welcome → resume → ai → browser (research skipped)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));

    expect(screen.queryByTestId(TEST_IDS.onboarding.stepResearch)).not.toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.onboarding.stepBrowser)).toBeInTheDocument();
  });
});

describe('OnboardingWizard — navigation', () => {
  it('advances from first step to second step on onNext', async () => {
    const user = userEvent.setup();
    renderWizard();

    expect(screen.getByTestId(TEST_IDS.onboarding.stepWelcome)).toBeInTheDocument();

    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));

    expect(screen.queryByTestId(TEST_IDS.onboarding.stepWelcome)).not.toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.onboarding.stepResume)).toBeInTheDocument();
  });

  it('stepIndex prop increments correctly on each onNext', async () => {
    const user = userEvent.setup();
    renderWizard();

    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepWelcome))).toBe(0);

    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepResume))).toBe(1);

    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepAi))).toBe(2);
  });

  it('renders SpotlightTour after onNext on the last step', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
    });
    const user = userEvent.setup();
    renderWizard();

    // 7-step sequence (openai): welcome(0) → resume(1) → ai(2) → browser(3) → adzunaKey(4) → extension(5) → appearance(6)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAppearance));

    expect(screen.getByTestId(TEST_IDS.onboarding.tour)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.onboarding.stepAppearance)).not.toBeInTheDocument();
  });

  it('calling onFinish on the tour marks onboarding complete (renders null)', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
    });
    const user = userEvent.setup();
    const { container } = renderWizard();

    // Advance through all 7 steps to reach the tour
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAppearance));

    // Tour is visible; click finish
    await user.click(screen.getByRole('button', { name: 'finish-tour' }));

    // Wizard should have unmounted — container children are empty
    expect(container.firstChild).toBeNull();
  });
});

describe('OnboardingWizard — sidebar force-open on tour start', () => {
  it('sets sidebarCollapsed to false only once the last step submits into the tour', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
      sidebarCollapsed: true,
    });
    const user = userEvent.setup();
    renderWizard();

    // 7-step sequence (openai): welcome(0) → resume(1) → ai(2) → browser(3) → adzunaKey(4) → extension(5) → appearance(6)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));

    // Still on a regular step — sidebar must be untouched.
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);

    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAppearance));

    // Tour now visible and the sidebar has been forced open so its
    // [data-tour-id] anchors exist for SpotlightTour to measure.
    expect(screen.getByTestId(TEST_IDS.onboarding.tour)).toBeInTheDocument();
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);
  });

  it('leaves sidebarCollapsed untouched while navigating earlier steps', async () => {
    usePreferencesStore.setState({ sidebarCollapsed: true });
    const user = userEvent.setup();
    renderWizard();

    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));

    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);
  });

  it('restores sidebarCollapsed to true once the tour finishes (was collapsed before onboarding)', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
      sidebarCollapsed: true,
    });
    const user = userEvent.setup();
    renderWizard();

    // 7-step sequence (openai): welcome(0) → resume(1) → ai(2) → browser(3) → adzunaKey(4) → extension(5) → appearance(6)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAppearance));

    // Tour forced the sidebar open (see the test above).
    expect(screen.getByTestId(TEST_IDS.onboarding.tour)).toBeInTheDocument();
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);

    // Finishing (or skipping) the tour must restore the user's original
    // collapsed preference instead of leaving it silently forced open.
    await user.click(screen.getByRole('button', { name: 'finish-tour' }));

    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);
  });

  it('leaves sidebarCollapsed false after the tour finishes for a first-run user (no-op restore)', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: {} },
      sidebarCollapsed: false,
    });
    const user = userEvent.setup();
    renderWizard();

    // 7-step sequence (openai): welcome(0) → resume(1) → ai(2) → browser(3) → adzunaKey(4) → extension(5) → appearance(6)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAppearance));

    expect(screen.getByTestId(TEST_IDS.onboarding.tour)).toBeInTheDocument();
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);

    await user.click(screen.getByRole('button', { name: 'finish-tour' }));

    // Default (never collapsed) — restoring is a no-op, stays false.
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);
  });
});

describe('OnboardingWizard — goBack floor', () => {
  it('does not crash when goBack is called at stepIndex 0', async () => {
    const user = userEvent.setup();
    renderWizard();

    // Advance to step 1, then go back to step 0
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    expect(screen.getByTestId(TEST_IDS.onboarding.stepResume)).toBeInTheDocument();

    // Go back to welcome
    await clickBack(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));

    expect(screen.getByTestId(TEST_IDS.onboarding.stepWelcome)).toBeInTheDocument();
    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepWelcome))).toBe(0);

    // Clicking the (non-existent / inert) back at index 0 must not crash.
    // The WelcomeStep stub only shows a back button when onBack is provided.
    // The wizard passes goBack unconditionally; verify the step renders fine.
    expect(screen.getByTestId(TEST_IDS.onboarding.stepWelcome)).toBeInTheDocument();
  });

  it('stepIndex stays at 0 when goBack is triggered at first step', async () => {
    const user = userEvent.setup();
    renderWizard();

    // Navigate forward then back to index 0
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickBack(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));

    // Now at index 0. The WelcomeStep stub only renders a back button when
    // onBack is provided; the wizard always passes goBack so the button IS
    // present. Assert it exists (non-vacuous), click it, and confirm the
    // wizard stays at index 0 — goBack is a floor-clamped no-op at step 0.
    const welcomeEl = screen.getByTestId(TEST_IDS.onboarding.stepWelcome);
    expect(stepIndexOf(welcomeEl)).toBe(0);

    const welcomeBackBtn = within(welcomeEl).queryByRole('button', { name: 'back' });
    expect(welcomeBackBtn).not.toBeNull();
    if (welcomeBackBtn) await user.click(welcomeBackBtn);

    // Identity of the visible step must not change
    expect(screen.getByTestId(TEST_IDS.onboarding.stepWelcome)).toBeInTheDocument();
    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepWelcome))).toBe(0);
  });
});

describe('OnboardingWizard — clamp on provider flip', () => {
  it('clamps stepIndex to new last index when provider flips from ollama to openai', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'ollama', providers: {} },
    });
    const user = userEvent.setup();
    renderWizard();

    // Advance to the last step of the 8-step ollama sequence (index 7 = appearance)
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepWelcome));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResume));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAi));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepResearch));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepBrowser));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepAdzunaKey));
    await clickNext(user, screen.getByTestId(TEST_IDS.onboarding.stepExtension));

    // At index 7 (appearance), totalSteps 8
    expect(screen.getByTestId(TEST_IDS.onboarding.stepAppearance)).toBeInTheDocument();
    expect(stepIndexOf(screen.getByTestId(TEST_IDS.onboarding.stepAppearance))).toBe(7);
    expect(totalStepsOf(screen.getByTestId(TEST_IDS.onboarding.stepAppearance))).toBe(8);

    // Flip provider to openai — array shrinks to 7 steps (max valid index = 6).
    // The clamp effect must land the wizard on step 6 = appearance.
    act(() => {
      usePreferencesStore.setState({
        aiProviderConfig: { activeProvider: 'openai', providers: {} },
      });
    });

    // The clamped visible step must be exactly appearance at index 6 / totalSteps 7.
    const visibleStep = document.querySelector('[data-total-steps]');
    if (!visibleStep) throw new Error('expected a visible step after provider flip');
    const visibleStepEl = visibleStep as HTMLElement;

    // Identity: must be the appearance stub (not a fallback to welcome at index 0)
    expect(visibleStepEl.getAttribute('data-testid')).toBe('step-appearance');
    // Exact clamped index — not just "within range"
    expect(stepIndexOf(visibleStepEl)).toBe(6);
    expect(totalStepsOf(visibleStepEl)).toBe(7);
  });
});

describe('OnboardingWizard — completion gate', () => {
  it('renders null immediately when onboardingCompleted is true', () => {
    usePreferencesStore.setState({ onboardingCompleted: true });
    const { container } = renderWizard();
    expect(container.firstChild).toBeNull();
  });

  it('renders the wizard when onboardingCompleted is false', () => {
    usePreferencesStore.setState({ onboardingCompleted: false });
    renderWizard();
    expect(screen.getByTestId(TEST_IDS.onboarding.stepWelcome)).toBeInTheDocument();
  });
});
