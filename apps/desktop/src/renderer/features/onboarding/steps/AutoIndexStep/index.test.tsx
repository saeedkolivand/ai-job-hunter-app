/**
 * AutoIndexStep — the step's own Back/Continue navigation (#1118).
 *
 * The step shipped with only the switch and the step dots. `OnboardingStepWrapper`
 * contributes nothing visible: it renders the dots and installs a global
 * Enter/Escape shortcut, and that shortcut deliberately stands down while a
 * control that owns its own activation has focus — which the switch does. A
 * mouse-only user, or a keyboard user who has just flicked the toggle, was
 * therefore stranded on the step. These tests pin the buttons themselves, not
 * the keyboard shortcut.
 *
 * `usePreferencesStore` is the REAL zustand store (localStorage-persisted, no
 * IPC), seeded per test via `setState` like the other renderer tests. The
 * `setAutoIndexOnUpload` action is swapped for a `vi.fn` wrapper that CALLS
 * THROUGH, so each test can assert both the value the step asked to persist and
 * the state the real action produced — `false` is also the default, so
 * "state is still false" on its own would pass even if nothing was ever written.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub: key-passthrough t() (mirrors AppearanceStep/index.test.tsx) ────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── component + store (real store — no module mock) ───────────────────────────

import { usePreferencesStore } from '@/store/preferences-store';

import { AutoIndexStep } from './index';

// Captured once at module scope, BEFORE the first beforeEach swaps the action
// out — reading it inside beforeEach would wrap the previous test's wrapper.
const realSetAutoIndexOnUpload = usePreferencesStore.getState().setAutoIndexOnUpload;

let setAutoIndexOnUpload = vi.fn(realSetAutoIndexOnUpload);

beforeEach(() => {
  setAutoIndexOnUpload = vi.fn(realSetAutoIndexOnUpload);
  usePreferencesStore.setState({ autoIndexOnUpload: false, setAutoIndexOnUpload });
});

function renderStep() {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const result = render(
    <AutoIndexStep onBack={onBack} onNext={onNext} direction={1} stepIndex={4} totalSteps={7} />
  );
  return { ...result, onNext, onBack };
}

const backButton = () => screen.getByRole('button', { name: 'onboarding.back' });
const continueButton = () => screen.getByRole('button', { name: 'onboarding.continue' });

describe('AutoIndexStep — step navigation', () => {
  it('renders both a Back and a Continue button', () => {
    renderStep();

    expect(backButton()).toBeInTheDocument();
    expect(continueButton()).toBeInTheDocument();
  });

  it('clicking Continue persists the untouched (off) default once and advances', async () => {
    const user = userEvent.setup();
    const { onNext, onBack } = renderStep();

    await user.click(continueButton());

    expect(setAutoIndexOnUpload).toHaveBeenCalledTimes(1);
    expect(setAutoIndexOnUpload).toHaveBeenCalledWith(false);
    expect(usePreferencesStore.getState().autoIndexOnUpload).toBe(false);
    expect(onNext).toHaveBeenCalledTimes(1);
    expect(onBack).not.toHaveBeenCalled();
  });

  it('clicking Continue after switching the toggle on persists true', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    const toggle = screen.getByRole('switch', { name: 'onboarding.autoIndex.toggleLabel' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-checked', 'true');

    // Flicking the switch is not the commit: the value is written on advance,
    // which is why leaving the step without a Continue button lost the choice.
    expect(setAutoIndexOnUpload).not.toHaveBeenCalled();
    expect(onNext).not.toHaveBeenCalled();

    await user.click(continueButton());

    expect(setAutoIndexOnUpload).toHaveBeenCalledTimes(1);
    expect(setAutoIndexOnUpload).toHaveBeenCalledWith(true);
    // Absolute anchor: the real action ran and moved the store off its default.
    expect(usePreferencesStore.getState().autoIndexOnUpload).toBe(true);
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('clicking Back goes back without persisting or advancing', async () => {
    const user = userEvent.setup();
    const { onNext, onBack } = renderStep();

    await user.click(backButton());

    expect(onBack).toHaveBeenCalledTimes(1);
    expect(onNext).not.toHaveBeenCalled();
    expect(setAutoIndexOnUpload).not.toHaveBeenCalled();
  });
});
