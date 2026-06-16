/**
 * ExtensionStep — render + interaction tests.
 *
 * Strategy:
 *  - @ajh/translations → key-passthrough t() so assertions never depend on
 *    actual translation strings, only on the i18n key structure.
 *  - useOpenExternal is mocked via the @/services barrel. The mock returns a
 *    spy-backed mutateAsync so we can assert on the URL without triggering IPC.
 *  - withProviders + createMockClient supply the React Query / AppClient tree
 *    that service hooks require.
 *  - Prop defaults: direction=1, stepIndex=4, totalSteps=6 (extension is step 5
 *    of 6 in the non-ollama path).
 *
 * noUncheckedIndexedAccess: no raw array indexing.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stub ──────────────────────────────────────────────────────────────
// useOpenExternal is the only service dependency in ExtensionStep.
// We provide a spy-backed mutateAsync; mutate is included for completeness.

const mutateAsyncSpy = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useOpenExternal: () => ({
    mutateAsync: mutateAsyncSpy,
    mutate: vi.fn(),
  }),
}));

// ── @ajh/ui ── spread actual so Button / motion helpers resolve correctly ────
// FloatingIcon + withDelay are used in the component; keeping the real module
// avoids mocking side-effects we don't care about.

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return { ...(actual as object) };
});

// ── component under test ──────────────────────────────────────────────────────

import { ExtensionStep } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

const CHROME_URL = 'https://chromewebstore.google.com/detail/oaoekkgkhmgdfnpmfkpphgiikliaicll';

function renderStep(overrides: Partial<Parameters<typeof ExtensionStep>[0]> = {}) {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const client = createMockClient();

  const props = {
    onNext,
    onBack,
    direction: 1 as const,
    stepIndex: 4,
    totalSteps: 6,
    ...overrides,
  };

  const result = render(<ExtensionStep {...props} />, { wrapper: withProviders(client) });
  return { ...result, onNext, onBack };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ExtensionStep — render', () => {
  it('renders the Add to Chrome button', () => {
    renderStep();
    // The button text is the i18n key (key-passthrough)
    expect(
      screen.getByRole('button', { name: 'onboarding.extension.addToChrome' })
    ).toBeInTheDocument();
  });

  it('renders the Firefox button in a disabled state', () => {
    renderStep();
    const firefoxBtn = screen.getByRole('button', {
      name: 'onboarding.extension.firefoxSoon',
    });
    expect(firefoxBtn).toBeInTheDocument();
    expect(firefoxBtn).toBeDisabled();
  });

  it('renders the next navigation button', () => {
    renderStep();
    expect(screen.getByRole('button', { name: 'onboarding.extension.next' })).toBeInTheDocument();
  });
});

describe('ExtensionStep — interaction', () => {
  it('clicking Add to Chrome calls openExternal mutation with the Chrome Web Store URL', async () => {
    mutateAsyncSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.extension.addToChrome' }));

    expect(mutateAsyncSpy).toHaveBeenCalledTimes(1);
    expect(mutateAsyncSpy).toHaveBeenCalledWith(CHROME_URL);
  });

  it('clicking the next button calls onNext', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.extension.next' }));

    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('clicking the back button calls onBack when onBack is provided', async () => {
    const user = userEvent.setup();
    const { onBack } = renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.extension.back' }));

    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('clicking the back button calls onBack exactly once (separate from the next-button spy)', async () => {
    const user = userEvent.setup();
    const { onNext, onBack } = renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.extension.back' }));

    expect(onBack).toHaveBeenCalledTimes(1);
    // Confirm the Next spy was NOT triggered by the Back click
    expect(onNext).not.toHaveBeenCalled();
  });
});
