/**
 * AdzunaKeyStep — render, key-save, skip, and connected-state tests.
 *
 * Strategy:
 *  - @ajh/translations → key-passthrough t() (mirrors ResearchStep pattern).
 *  - @/services is mocked: useHasProviderKey returns configurable per-slot data;
 *    useSetProviderKey returns a mutateAsync spy; useOpenExternal is a no-op.
 *  - @ajh/ui is spread from the real module; useNotification is replaced with spies.
 *  - withProviders + createMockClient supply React Query / AppClient context.
 *
 * noUncheckedIndexedAccess: all array accesses are guarded.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stubs ─────────────────────────────────────────────────────────────

let stubIdHas = false;
let stubKeyHas = false;
const mutateAsyncSpy = vi.fn().mockResolvedValue(undefined);
const notifySuccessSpy = vi.fn();
const notifyErrorSpy = vi.fn();

beforeEach(() => {
  stubIdHas = false;
  stubKeyHas = false;
  mutateAsyncSpy.mockClear();
  notifySuccessSpy.mockClear();
  notifyErrorSpy.mockClear();
});

vi.mock('@/services', () => ({
  useHasProviderKey: (slot: string) => {
    if (slot === 'adzuna-app-id') return { data: { has: stubIdHas } };
    if (slot === 'adzuna-app-key') return { data: { has: stubKeyHas } };
    return { data: { has: false } };
  },
  useSetProviderKey: () => ({ mutateAsync: mutateAsyncSpy }),
  useOpenExternal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
}));

// ── @ajh/ui: spread real module; stub useNotification ─────────────────────────

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    useNotification: () => ({
      success: notifySuccessSpy,
      error: notifyErrorSpy,
    }),
  };
});

// ── component under test (imported AFTER mocks) ───────────────────────────────

import { AdzunaKeyStep } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderStep(overrides: Partial<Parameters<typeof AdzunaKeyStep>[0]> = {}) {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const client = createMockClient();

  const props = {
    onNext,
    onBack,
    direction: 1 as const,
    stepIndex: 4,
    totalSteps: 7,
    ...overrides,
  };

  const result = render(<AdzunaKeyStep {...props} />, { wrapper: withProviders(client) });
  return { ...result, onNext, onBack };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AdzunaKeyStep — render', () => {
  it('renders the title key', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();
    expect(screen.getByText('onboarding.adzunaKey.title')).toBeInTheDocument();
  });

  it('renders the subtitle key', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();
    expect(screen.getByText('onboarding.adzunaKey.subtitle')).toBeInTheDocument();
  });

  it('renders App ID and App Key input fields when neither key is saved', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();
    expect(screen.getByLabelText('onboarding.adzunaKey.appIdLabel')).toBeInTheDocument();
    expect(screen.getByLabelText('onboarding.adzunaKey.appKeyLabel')).toBeInTheDocument();
  });

  it('renders the "skip" button label when neither key is saved', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();
    // The next/skip button is the forward button; label is skip when not both saved
    expect(
      screen.getByRole('button', { name: /onboarding\.adzunaKey\.skip/i })
    ).toBeInTheDocument();
  });

  it('renders the skip hint text', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();
    expect(screen.getByText('onboarding.adzunaKey.skipHint')).toBeInTheDocument();
  });
});

describe('AdzunaKeyStep — connected state', () => {
  it('shows the connected banner when both keys are saved', () => {
    stubIdHas = true;
    stubKeyHas = true;
    renderStep();
    expect(screen.getByText('onboarding.adzunaKey.connected')).toBeInTheDocument();
  });

  it('hides the input fields when both keys are saved', () => {
    stubIdHas = true;
    stubKeyHas = true;
    renderStep();
    expect(screen.queryByLabelText('onboarding.adzunaKey.appIdLabel')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('onboarding.adzunaKey.appKeyLabel')).not.toBeInTheDocument();
  });

  it('shows "next" label (not "skip") when both keys are saved', () => {
    stubIdHas = true;
    stubKeyHas = true;
    renderStep();
    expect(
      screen.getByRole('button', { name: /onboarding\.adzunaKey\.next/i })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /onboarding\.adzunaKey\.skip/i })
    ).not.toBeInTheDocument();
  });

  it('shows partial-saved hint when only App ID is saved', () => {
    stubIdHas = true;
    stubKeyHas = false;
    renderStep();
    expect(screen.getByText('onboarding.adzunaKey.partialSaved')).toBeInTheDocument();
  });
});

describe('AdzunaKeyStep — key save', () => {
  it('calls useSetProviderKey mutateAsync with adzuna-app-id slot when App ID is typed and saved', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    mutateAsyncSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    const appIdInput = screen.getByLabelText('onboarding.adzunaKey.appIdLabel');
    await user.type(appIdInput, 'my-app-id');

    const saveBtn = screen.getByRole('button', { name: /onboarding\.adzunaKey\.save/i });
    await user.click(saveBtn);

    const calls = mutateAsyncSpy.mock.calls;
    const idCall = calls.find(
      (c) =>
        (c[0] as { provider: string; apiKey: string } | undefined)?.provider === 'adzuna-app-id'
    );
    expect(idCall).toBeDefined();
    const arg = idCall?.[0] as { provider: string; apiKey: string } | undefined;
    expect(arg?.apiKey).toBe('my-app-id');
  });

  it('calls useSetProviderKey mutateAsync with adzuna-app-key slot when App Key is typed and saved', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    mutateAsyncSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    const appKeyInput = screen.getByLabelText('onboarding.adzunaKey.appKeyLabel');
    await user.type(appKeyInput, 'my-app-key');

    const saveBtn = screen.getByRole('button', { name: /onboarding\.adzunaKey\.save/i });
    await user.click(saveBtn);

    const calls = mutateAsyncSpy.mock.calls;
    const keyCall = calls.find(
      (c) =>
        (c[0] as { provider: string; apiKey: string } | undefined)?.provider === 'adzuna-app-key'
    );
    expect(keyCall).toBeDefined();
    const arg = keyCall?.[0] as { provider: string; apiKey: string } | undefined;
    expect(arg?.apiKey).toBe('my-app-key');
  });

  it('shows a success notification after saving', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    mutateAsyncSpy.mockClear();
    notifySuccessSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    await user.type(screen.getByLabelText('onboarding.adzunaKey.appIdLabel'), 'id');
    await user.click(screen.getByRole('button', { name: /onboarding\.adzunaKey\.save/i }));

    expect(notifySuccessSpy).toHaveBeenCalledTimes(1);
  });

  it('shows an error notification when mutateAsync rejects', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    mutateAsyncSpy.mockRejectedValueOnce(new Error('network fail'));
    notifyErrorSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    await user.type(screen.getByLabelText('onboarding.adzunaKey.appIdLabel'), 'id');
    await user.click(screen.getByRole('button', { name: /onboarding\.adzunaKey\.save/i }));

    expect(notifyErrorSpy).toHaveBeenCalledTimes(1);
    const call = notifyErrorSpy.mock.calls[0];
    const arg = call?.[0] as { message: string } | undefined;
    expect(arg?.message).toBe('network fail');
  });
});

describe('AdzunaKeyStep — navigation', () => {
  it('clicking skip/next calls onNext', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.click(screen.getByRole('button', { name: /onboarding\.adzunaKey\.skip/i }));

    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('clicking back calls onBack', async () => {
    stubIdHas = false;
    stubKeyHas = false;
    const user = userEvent.setup();
    const { onBack } = renderStep();

    await user.click(screen.getByRole('button', { name: /onboarding\.adzunaKey\.back/i }));

    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('skip/next is always enabled (step is always skippable)', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderStep();

    const skipBtn = screen.getByRole('button', { name: /onboarding\.adzunaKey\.skip/i });
    expect(skipBtn).not.toBeDisabled();
  });
});
