/**
 * WelcomeStep — Enter-key / Continue-click parity (same defect class fixed in
 * AISelectionStep, #936).
 *
 * The regression this guards: `OnboardingStepWrapper`'s global Enter-key
 * listener calls whatever `onNext` prop it's handed directly — bypassing
 * `handleNext`'s `setUserName(trimmed)` write. The name `<Input>`'s own
 * keydown handler `stopPropagation()`s Enter while focus is INSIDE it, so the
 * click path always worked and masked the bug; but move focus OUT of the
 * input (e.g. Tab to a language tile) and press Enter, and the wrapper's
 * listener fires directly — advancing without ever saving the typed name.
 * `WelcomeStep` must wire `onNext={handleNext}`, not the raw `onNext` prop,
 * so both paths persist the name identically.
 *
 * `usePreferencesStore` is the REAL zustand store (localStorage-persisted, no
 * IPC) — reset via `resetPreferences()` in beforeEach and asserted via
 * `getState()`, mirroring ApplicantDetailsSection.test.tsx rather than
 * mocking the store.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub ─────────────────────────────────────────────────────────────────
// WelcomeStep imports the `@/i18n` shim's DEFAULT export directly (not just
// the `useTranslation` hook) for the language-tile `i18n.changeLanguage` /
// `i18n.language` reads — that shim re-exports `@ajh/translations`'s default
// and calls `.on('languageChanged', …)` as a side effect, so the mock needs a
// minimal stand-in for both, not just `useTranslation`.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
  default: { on: vi.fn(), changeLanguage: vi.fn(), language: 'en' },
}));

// ── component + store (real store — no mock) ──────────────────────────────────

import { usePreferencesStore } from '@/store/preferences-store';

import { WelcomeStep } from './index';

function renderStep() {
  const onNext = vi.fn();
  const result = render(<WelcomeStep onNext={onNext} direction={1} stepIndex={0} totalSteps={7} />);
  return { ...result, onNext };
}

beforeEach(() => {
  usePreferencesStore.getState().resetPreferences();
});

describe('WelcomeStep — clicking Continue persists the name', () => {
  it('trims and saves the name via setUserName before advancing', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.type(
      screen.getByPlaceholderText('onboarding.welcome.namePlaceholder'),
      '  Ada Lovelace  '
    );
    await user.click(screen.getByRole('button', { name: /onboarding\.welcome\.next/ }));

    expect(usePreferencesStore.getState().userName).toBe('Ada Lovelace');
    expect(onNext).toHaveBeenCalledTimes(1);
  });
});

describe('WelcomeStep — Enter with focus moved OUT of the input takes the identical path as clicking Continue', () => {
  it('still persists the typed name when Enter fires after focus moves to a language tile', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.type(
      screen.getByPlaceholderText('onboarding.welcome.namePlaceholder'),
      'Grace Hopper'
    );

    // Tab OUT of the name input onto a language tile — the input's own
    // keydown handler no longer intercepts Enter once focus has moved, so
    // this reaches OnboardingStepWrapper's global window listener instead.
    await user.tab();
    expect(document.activeElement).not.toBeInstanceOf(HTMLInputElement);

    await user.keyboard('{Enter}');

    expect(usePreferencesStore.getState().userName).toBe('Grace Hopper');
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('does nothing when Enter fires with no name typed — canAdvance still gates the keyboard path', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    screen.getByPlaceholderText('onboarding.welcome.namePlaceholder').focus();
    await user.tab();
    await user.keyboard('{Enter}');

    expect(usePreferencesStore.getState().userName).toBe('');
    expect(onNext).not.toHaveBeenCalled();
  });
});
