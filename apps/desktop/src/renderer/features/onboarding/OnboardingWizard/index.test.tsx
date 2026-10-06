/**
 * OnboardingWizard — step filter, navigation, clamp, and completion-gate tests.
 *
 * Strategy:
 *  - All step components are stubbed to lightweight buttons that expose their
 *    props (stepIndex, totalSteps, onNext, onBack) via data-testid attributes
 *    (see `test-mocks`). This keeps the filter/clamp/nav logic under test
 *    without dragging in every step's service dependencies.
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

import type { AiProvider } from '@/store/preferences-schema';
import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, withProviders } from '@/test-support';

import { OnboardingWizard } from './index';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Active provider — backend-owned (task #16), read via useActiveConfig. Back
// it with the Zustand store here so the existing tests keep driving the provider
// via `usePreferencesStore.setState({ aiProviderConfig })` (synchronous + reactive,
// so the mid-test clamp flip re-renders in place).

vi.mock('@/services', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  const { usePreferencesStore: store } = await import('@/store/preferences-store');
  return {
    ...(actual as object),
    useActiveConfig: () => {
      const cfg = store((s) => s.aiProviderConfig);
      return {
        data: cfg
          ? { activeProvider: cfg.activeProvider, providers: cfg.providers ?? {} }
          : { providers: {} },
        isPending: false,
      };
    },
  };
});

// ── Step component + tour stubs (factories live in ./test-mocks) ──────────────

vi.mock('../steps/WelcomeStep', async () => ({
  WelcomeStep: (await import('./test-mocks')).stepStub('stepWelcome'),
}));
vi.mock('../steps/ResumeStep', async () => ({
  ResumeStep: (await import('./test-mocks')).stepStub('stepResume'),
}));
vi.mock('../steps/AISelectionStep', async () => ({
  AISelectionStep: (await import('./test-mocks')).stepStub('stepAi'),
}));
vi.mock('../steps/ResearchStep', async () => ({
  ResearchStep: (await import('./test-mocks')).stepStub('stepResearch'),
}));
vi.mock('../steps/BrowserStep', async () => ({
  BrowserStep: (await import('./test-mocks')).stepStub('stepBrowser'),
}));
vi.mock('../steps/AdzunaKeyStep', async () => ({
  AdzunaKeyStep: (await import('./test-mocks')).stepStub('stepAdzunaKey'),
}));
vi.mock('../steps/ExtensionStep', async () => ({
  ExtensionStep: (await import('./test-mocks')).stepStub('stepExtension'),
}));
vi.mock('../steps/AutoIndexStep', async () => ({
  AutoIndexStep: (await import('./test-mocks')).stepStub('stepAutoIndex'),
}));
vi.mock('../steps/CrashReportingStep', async () => ({
  CrashReportingStep: (await import('./test-mocks')).stepStub('stepCrashReporting'),
}));
vi.mock('../steps/AppearanceStep', async () => ({
  AppearanceStep: (await import('./test-mocks')).stepStub('stepAppearance'),
}));
vi.mock('../SpotlightTour', async () => ({
  SpotlightTour: (await import('./test-mocks')).tourStub,
}));

// ── helpers ───────────────────────────────────────────────────────────────────

type StepId = keyof typeof TEST_IDS.onboarding;
type User = ReturnType<typeof userEvent.setup>;

/** The step order for a non-ollama provider (no research step). */
const OPENAI_FLOW: StepId[] = [
  'stepWelcome',
  'stepResume',
  'stepAi',
  'stepBrowser',
  'stepAdzunaKey',
  'stepExtension',
  'stepAutoIndex',
  'stepCrashReporting',
  'stepAppearance',
];

/** The same order for ollama, which adds the research step after the AI step. */
const OLLAMA_FLOW: StepId[] = [...OPENAI_FLOW.slice(0, 3), 'stepResearch', ...OPENAI_FLOW.slice(3)];

const stepEl = (id: StepId) => screen.getByTestId(TEST_IDS.onboarding[id]);

function renderWizard() {
  const client = createMockClient();
  return render(<OnboardingWizard />, { wrapper: withProviders(client) });
}

/** Seed the backend-owned active provider (the wizard reads it through the store). */
const setProvider = (activeProvider: AiProvider, extra: Record<string, unknown> = {}) =>
  usePreferencesStore.setState({
    aiProviderConfig: { activeProvider, providers: {} },
    ...extra,
  });

/** Return the data-total-steps attribute of the currently visible step. */
function totalStepsOf(el: HTMLElement): number {
  return Number(el.getAttribute('data-total-steps'));
}

function stepIndexOf(el: HTMLElement): number {
  return Number(el.getAttribute('data-step-index'));
}

/** Click the "next" button inside a step stub element. */
async function clickNext(user: User, stepElement: HTMLElement) {
  await user.click(within(stepElement).getByRole('button', { name: 'next' }));
}

/** Click the "back" button inside a step stub element. */
async function clickBack(user: User, stepElement: HTMLElement) {
  await user.click(within(stepElement).getByRole('button', { name: 'back' }));
}

/** Click "next" on the first `count` steps of `flow` (default: all of them, into the tour). */
async function advance(user: User, count = OPENAI_FLOW.length, flow = OPENAI_FLOW) {
  for (const id of flow.slice(0, count)) await clickNext(user, stepEl(id));
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
  it('includes research step (10 total) when activeProvider is ollama', () => {
    setProvider('ollama');
    renderWizard();

    expect(totalStepsOf(stepEl('stepWelcome'))).toBe(10);
  });

  it('excludes research step (9 total) when activeProvider is openai', () => {
    setProvider('openai');
    renderWizard();

    expect(totalStepsOf(stepEl('stepWelcome'))).toBe(9);
  });

  it('excludes research step (9 total) when activeProvider is undefined', () => {
    usePreferencesStore.setState({ aiProviderConfig: undefined });
    renderWizard();

    expect(totalStepsOf(stepEl('stepWelcome'))).toBe(9);
  });

  it('research stub is present in the DOM when ollama is active after navigating to it', async () => {
    setProvider('ollama');
    const user = userEvent.setup();
    renderWizard();

    // welcome → resume → ai → research (index 3)
    await advance(user, 3, OLLAMA_FLOW);

    expect(stepEl('stepResearch')).toBeInTheDocument();
  });

  it('research stub never appears when activeProvider is openai', async () => {
    setProvider('openai');
    const user = userEvent.setup();
    renderWizard();

    // welcome → resume → ai → browser (research skipped)
    await advance(user, 3);

    expect(screen.queryByTestId(TEST_IDS.onboarding.stepResearch)).not.toBeInTheDocument();
    expect(stepEl('stepBrowser')).toBeInTheDocument();
  });
});

describe('OnboardingWizard — navigation', () => {
  it('advances from first step to second step on onNext', async () => {
    const user = userEvent.setup();
    renderWizard();

    expect(stepEl('stepWelcome')).toBeInTheDocument();

    await clickNext(user, stepEl('stepWelcome'));

    expect(screen.queryByTestId(TEST_IDS.onboarding.stepWelcome)).not.toBeInTheDocument();
    expect(stepEl('stepResume')).toBeInTheDocument();
  });

  it('stepIndex prop increments correctly on each onNext', async () => {
    const user = userEvent.setup();
    renderWizard();

    expect(stepIndexOf(stepEl('stepWelcome'))).toBe(0);

    await clickNext(user, stepEl('stepWelcome'));
    expect(stepIndexOf(stepEl('stepResume'))).toBe(1);

    await clickNext(user, stepEl('stepResume'));
    expect(stepIndexOf(stepEl('stepAi'))).toBe(2);
  });

  it('renders SpotlightTour after onNext on the last step', async () => {
    setProvider('openai');
    const user = userEvent.setup();
    renderWizard();

    // openai sequence: welcome(0) → resume(1) → ai(2) → browser(3) → adzunaKey(4)
    // → extension(5) → autoIndex(6) → crashReporting(7) → appearance(8)
    await advance(user);

    expect(stepEl('tour')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.onboarding.stepAppearance)).not.toBeInTheDocument();
  });

  it('calling onFinish on the tour marks onboarding complete (renders null)', async () => {
    setProvider('openai');
    const user = userEvent.setup();
    const { container } = renderWizard();

    // Advance through every step to reach the tour
    await advance(user);

    // Tour is visible; click finish
    await user.click(screen.getByRole('button', { name: 'finish-tour' }));

    // Wizard should have unmounted — container children are empty
    expect(container.firstChild).toBeNull();
  });
});

describe('OnboardingWizard — sidebar force-open on tour start', () => {
  it('sets sidebarCollapsed to false only once the last step submits into the tour', async () => {
    setProvider('openai', { sidebarCollapsed: true });
    const user = userEvent.setup();
    renderWizard();

    // openai sequence: welcome(0) → … → crashReporting(7) → appearance(8)
    await advance(user, OPENAI_FLOW.length - 1);

    // Still on a regular step — sidebar must be untouched.
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);

    await clickNext(user, stepEl('stepAppearance'));

    // Tour now visible and the sidebar has been forced open so its
    // [data-tour-id] anchors exist for SpotlightTour to measure.
    expect(stepEl('tour')).toBeInTheDocument();
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);
  });

  it('leaves sidebarCollapsed untouched while navigating earlier steps', async () => {
    usePreferencesStore.setState({ sidebarCollapsed: true });
    const user = userEvent.setup();
    renderWizard();

    await clickNext(user, stepEl('stepWelcome'));

    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);
  });

  it('restores sidebarCollapsed to true once the tour finishes (was collapsed before onboarding)', async () => {
    setProvider('openai', { sidebarCollapsed: true });
    const user = userEvent.setup();
    renderWizard();

    await advance(user);

    // Tour forced the sidebar open (see the test above).
    expect(stepEl('tour')).toBeInTheDocument();
    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(false);

    // Finishing (or skipping) the tour must restore the user's original
    // collapsed preference instead of leaving it silently forced open.
    await user.click(screen.getByRole('button', { name: 'finish-tour' }));

    expect(usePreferencesStore.getState().sidebarCollapsed).toBe(true);
  });

  it('leaves sidebarCollapsed false after the tour finishes for a first-run user (no-op restore)', async () => {
    setProvider('openai', { sidebarCollapsed: false });
    const user = userEvent.setup();
    renderWizard();

    await advance(user);

    expect(stepEl('tour')).toBeInTheDocument();
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
    await clickNext(user, stepEl('stepWelcome'));
    expect(stepEl('stepResume')).toBeInTheDocument();

    // Go back to welcome
    await clickBack(user, stepEl('stepResume'));

    expect(stepEl('stepWelcome')).toBeInTheDocument();
    expect(stepIndexOf(stepEl('stepWelcome'))).toBe(0);

    // Clicking the (non-existent / inert) back at index 0 must not crash.
    // The WelcomeStep stub only shows a back button when onBack is provided.
    // The wizard passes goBack unconditionally; verify the step renders fine.
    expect(stepEl('stepWelcome')).toBeInTheDocument();
  });

  it('stepIndex stays at 0 when goBack is triggered at first step', async () => {
    const user = userEvent.setup();
    renderWizard();

    // Navigate forward then back to index 0
    await clickNext(user, stepEl('stepWelcome'));
    await clickBack(user, stepEl('stepResume'));

    // Now at index 0. The WelcomeStep stub only renders a back button when
    // onBack is provided; the wizard always passes goBack so the button IS
    // present. Assert it exists (non-vacuous), click it, and confirm the
    // wizard stays at index 0 — goBack is a floor-clamped no-op at step 0.
    const welcomeEl = stepEl('stepWelcome');
    expect(stepIndexOf(welcomeEl)).toBe(0);

    const welcomeBackBtn = within(welcomeEl).queryByRole('button', { name: 'back' });
    expect(welcomeBackBtn).not.toBeNull();
    if (welcomeBackBtn) await user.click(welcomeBackBtn);

    // Identity of the visible step must not change
    expect(stepEl('stepWelcome')).toBeInTheDocument();
    expect(stepIndexOf(stepEl('stepWelcome'))).toBe(0);
  });
});

describe('OnboardingWizard — clamp on provider flip', () => {
  it('clamps stepIndex to new last index when provider flips from ollama to openai', async () => {
    setProvider('ollama');
    const user = userEvent.setup();
    renderWizard();

    // Advance to the last step of the 10-step ollama sequence (index 9 = appearance)
    await advance(user, OLLAMA_FLOW.length - 1, OLLAMA_FLOW);

    // At index 9 (appearance), totalSteps 10
    expect(stepEl('stepAppearance')).toBeInTheDocument();
    expect(stepIndexOf(stepEl('stepAppearance'))).toBe(9);
    expect(totalStepsOf(stepEl('stepAppearance'))).toBe(10);

    // Flip provider to openai — array shrinks to 9 steps (max valid index = 8).
    // The clamp effect must land the wizard on step 8 = appearance. `useActiveConfig`
    // is backed by the Zustand store in this test, so the flip is a plain setState.
    act(() => {
      setProvider('openai');
    });

    // The clamped visible step must be exactly appearance at index 8 / totalSteps 9.
    const visibleStep = document.querySelector('[data-total-steps]');
    if (!visibleStep) throw new Error('expected a visible step after provider flip');
    const visibleStepEl = visibleStep as HTMLElement;

    // Identity: must be the appearance stub (not a fallback to welcome at index 0)
    expect(visibleStepEl.getAttribute('data-testid')).toBe('step-appearance');
    // Exact clamped index — not just "within range"
    expect(stepIndexOf(visibleStepEl)).toBe(8);
    expect(totalStepsOf(visibleStepEl)).toBe(9);
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
    expect(stepEl('stepWelcome')).toBeInTheDocument();
  });
});
