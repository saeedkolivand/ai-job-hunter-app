/**
 * WelcomeStep — Enter-key behaviour across every focus target in the step
 * (same defect class first fixed in AISelectionStep, #936; hardened in #939).
 *
 * `OnboardingStepWrapper`'s global Enter-key listener is the shared
 * "advance the step" shortcut for all ten onboarding steps. Two things can go
 * wrong with a single global listener like that, and this file guards both:
 *
 *  1. Enter reaching the input must still save the name before advancing
 *     (`handleNext`'s `setUserName(trimmed)` write) — the ORIGINAL regression.
 *  2. Enter reaching a DIFFERENT focused control (a language tile, or the
 *     Continue button itself) must not be stolen by the wrapper's global
 *     shortcut: a focused control that owns its own Enter/click activation
 *     (button, link, select, role="button") gets to handle it alone. The
 *     wrapper only advances when canAdvance is set AND focus isn't on such a
 *     control. Getting this wrong two different ways: (a) letting the
 *     wrapper ALSO fire onNext when Continue is focused double-advances,
 *     skipping a step (Continue's own click already calls onNext); (b)
 *     excluding Continue but blanket-excluding EVERY button instead of just
 *     honouring each control's own activation would (if done via
 *     preventDefault) silently swallow a language tile's own click and
 *     prevent it from ever picking a language via keyboard.
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

describe('WelcomeStep — Enter while typing in the name input takes the identical path as clicking Continue', () => {
  it('persists the typed name and calls onNext exactly once when Enter fires WITHOUT tabbing out — the most common path', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    // No `user.tab()` here: this is the input's own `handleKeyDown` ->
    // `stopPropagation()` -> `handleNext()` path, guarded separately from
    // OnboardingStepWrapper's window listener. Asserting the exact call
    // count (not just "was called") is what locks the double-fire guard in:
    // if `stopPropagation()` is ever removed, the wrapper's own focused-input
    // exclusion is the only thing left standing between this and a
    // double-advance.
    await user.type(
      screen.getByPlaceholderText('onboarding.welcome.namePlaceholder'),
      'Grace Hopper{Enter}'
    );

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

  it('calls onNext exactly once when Enter fires with the Continue button itself focused (#939 double-fire)', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.type(
      screen.getByPlaceholderText('onboarding.welcome.namePlaceholder'),
      'Grace Hopper'
    );

    // Focus the Continue button directly. Enter here triggers BOTH
    // OnboardingStepWrapper's window keydown listener AND the browser's
    // native Enter-on-a-focused-button synthesized click, which fires the
    // button's own onClick={handleNext}. The wrapper recognizes a focused
    // <button> as owning its own activation and skips its OWN onNext call,
    // so only the button's native click goes through — exactly once.
    const continueButton = screen.getByRole('button', { name: /onboarding\.welcome\.next/ });
    continueButton.focus();
    expect(continueButton).toHaveFocus();

    await user.keyboard('{Enter}');

    expect(usePreferencesStore.getState().userName).toBe('Grace Hopper');
    expect(onNext).toHaveBeenCalledTimes(1);
  });
});

describe('WelcomeStep — Enter on a different focused control activates THAT control, not the wizard', () => {
  it('picks the focused language tile on Enter and does NOT advance the step', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.type(
      screen.getByPlaceholderText('onboarding.welcome.namePlaceholder'),
      'Grace Hopper'
    );

    // Tab past the input onto the language tiles, landing on Deutsch. Enter
    // here must activate THAT tile (select German) the same way a click on
    // it would — not fall through to the wrapper's global "advance"
    // shortcut, which would silently discard the user's keyboard selection.
    await user.tab(); // -> English tile
    await user.tab(); // -> Deutsch tile
    expect(screen.getByRole('button', { name: /Deutsch/ })).toHaveFocus();

    await user.keyboard('{Enter}');

    // The tile's own click fired (language changed via the real store)...
    expect(usePreferencesStore.getState().language).toBe('de');
    // ...and the wrapper's global shortcut did NOT also fire: the typed name
    // was never saved (that only happens via handleNext) and onNext was
    // never called.
    expect(usePreferencesStore.getState().userName).toBe('');
    expect(onNext).not.toHaveBeenCalled();
  });
});
