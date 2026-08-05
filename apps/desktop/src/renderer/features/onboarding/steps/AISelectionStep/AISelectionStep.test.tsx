/**
 * AISelectionStep — cloud-mode gating tests (live-model-lists PR).
 *
 * The regression this guards: onboarding used to pre-select a hardcoded cloud
 * model id (`CLOUD_DEFAULT_MODELS`) the instant a key was saved — the exact
 * defect class that shipped a shut-down `gemini-2.0-flash` as the Gemini
 * default. Model choice is now deferred to the live list (via the real,
 * unmocked `CloudProviderPanel` + `Dropdown`):
 *  - Continue stays disabled in cloud mode until a model is picked.
 *  - `handleContinue` configures the provider with the PICKED model, never a
 *    hardcoded one.
 *  - Switching cloud provider resets the pick (a model id from a different
 *    provider's catalogue must not carry over) and re-disables Continue.
 *
 * `@/services` is mocked wholesale (both AISelectionStep's own hooks and the
 * ones the real child panels — CloudProviderPanel, OllamaNotInstalled — call),
 * `@ajh/shared`'s `getRecommended`/`MODEL_RECS` are the real, pure exports.
 * `motion/react` is globally mocked (vitest.setup.ts) to render children
 * synchronously, so `AnimatePresence` mode-switches don't need fake timers.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @/services stub — covers AISelectionStep + the real CloudProviderPanel/
// OllamaNotInstalled children it renders ───────────────────────────────────────

let stubHasKey = false;
type ModelsQueryState = {
  data?: { models: Array<{ name: string; displayName?: string }>; cached: boolean };
  isLoading: boolean;
  isError: boolean;
  error?: unknown;
};
let stubModelsQuery: ModelsQueryState = { data: undefined, isLoading: false, isError: false };

const configureMutateSpy = vi.fn();

// The real `CloudProviderPanel` (rendered unmocked) calls `useNotification` —
// stub it (mirrors AdzunaKeyStep's pattern) rather than mounting a full
// NotificationProvider tree.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    useNotification: () => ({
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
      info: vi.fn(),
      open: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

vi.mock('@/services', () => ({
  useActiveConfig: () => ({ data: { activeProvider: 'ollama', providers: {} } }),
  useAIModels: () => ({ data: [] }),
  useConfigureActiveProvider: () => ({ mutate: configureMutateSpy }),
  useSystemHealth: () => ({ data: { ai: { ready: false }, cliAgents: {} }, isLoading: false }),
  useSystemResources: () => ({
    resources: {
      totalRamGb: 8,
      usedRamGb: 4,
      freeRamGb: 4,
      hasGpu: false,
      totalVramGb: 0,
      usedVramGb: 0,
      freeVramGb: 0,
      deviceTier: { label: 'Mid', color: 'text-blue-400' },
    },
    modelUsage: { tooHeavy: false },
  }),
  useHasProviderKey: () => ({ data: { has: stubHasKey } }),
  useListProviderModels: () => stubModelsQuery,
  useOpenExternal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useSetProviderKey: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useTestProviderKey: () => ({ mutateAsync: vi.fn().mockResolvedValue({ success: true }) }),
}));

// ── component under test ──────────────────────────────────────────────────────

import { AISelectionStep } from './index';

function renderStep() {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const result = render(
    <AISelectionStep onBack={onBack} onNext={onNext} direction={1} stepIndex={2} totalSteps={5} />
  );
  return { ...result, onNext, onBack };
}

async function goToCloudTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'onboarding.ai.cloudTab' }));
}

beforeEach(() => {
  stubHasKey = false;
  stubModelsQuery = { data: undefined, isLoading: false, isError: false };
  configureMutateSpy.mockClear();
});

describe('AISelectionStep — cloud mode, no model picked yet', () => {
  it('disables Continue when no key is stored', async () => {
    const user = userEvent.setup();
    renderStep();
    await goToCloudTab(user);

    expect(screen.getByRole('button', { name: /onboarding\.ai\.next/ })).toBeDisabled();
  });

  it('ties the disabled Continue button to the pick-a-model hint via aria-describedby', async () => {
    const user = userEvent.setup();
    renderStep();
    await goToCloudTab(user);

    const continueBtn = screen.getByRole('button', { name: /onboarding\.ai\.next/ });
    const describedBy = continueBtn.getAttribute('aria-describedby');
    expect(describedBy).toBeTruthy();
    const hint = document.getElementById(describedBy ?? '');
    expect(hint).toHaveTextContent('onboarding.ai.pickModelHint');
  });

  it('still disables Continue once a key is stored but no model is picked', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }, { name: 'o1' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    renderStep();
    await goToCloudTab(user);

    expect(screen.getByRole('button', { name: /onboarding\.ai\.next/ })).toBeDisabled();
  });
});

describe('AISelectionStep — cloud mode, model picked from the live list', () => {
  it('enables Continue and configures the PICKED model, never a hardcoded default', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }, { name: 'o1' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    const { onNext } = renderStep();
    await goToCloudTab(user);

    await user.click(screen.getByRole('button', { name: 'onboarding.ai.selectModelPlaceholder' }));
    await user.click(screen.getByRole('option', { name: 'o1' }));

    const continueBtn = screen.getByRole('button', { name: /onboarding\.ai\.next/ });
    expect(continueBtn).not.toBeDisabled();

    await user.click(continueBtn);

    expect(configureMutateSpy).toHaveBeenCalledWith({ provider: 'openai', model: 'o1' });
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('drops the aria-describedby hint once a model is picked and Continue is enabled', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    renderStep();
    await goToCloudTab(user);

    await user.click(screen.getByRole('button', { name: 'onboarding.ai.selectModelPlaceholder' }));
    await user.click(screen.getByRole('option', { name: 'gpt-4o' }));

    const continueBtn = screen.getByRole('button', { name: /onboarding\.ai\.next/ });
    expect(continueBtn).not.toHaveAttribute('aria-describedby');
  });
});

describe('AISelectionStep — Enter key takes the identical path as clicking Continue', () => {
  // `OnboardingStepWrapper`'s Enter-key handler calls the `onNext` prop
  // directly, bypassing whatever configures the provider — it must receive
  // `handleContinue`, not the raw `onNext` prop, or Enter silently discards a
  // model the user just deliberately picked from a live catalogue.
  it('pressing Enter with a model picked configures the provider with the PICKED model', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }, { name: 'o1' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    const { onNext } = renderStep();
    await goToCloudTab(user);

    await user.click(screen.getByRole('button', { name: 'onboarding.ai.selectModelPlaceholder' }));
    await user.click(screen.getByRole('option', { name: 'o1' }));

    await user.keyboard('{Enter}');

    expect(configureMutateSpy).toHaveBeenCalledWith({ provider: 'openai', model: 'o1' });
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('pressing Enter with no model picked does neither — canAdvance still gates the keyboard path', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    const { onNext } = renderStep();
    await goToCloudTab(user);

    await user.keyboard('{Enter}');

    expect(configureMutateSpy).not.toHaveBeenCalled();
    expect(onNext).not.toHaveBeenCalled();
  });
});

describe('AISelectionStep — switching cloud provider resets the pick', () => {
  it('re-disables Continue after switching provider, even though a model was picked for the old one', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    renderStep();
    await goToCloudTab(user);

    await user.click(screen.getByRole('button', { name: 'onboarding.ai.selectModelPlaceholder' }));
    await user.click(screen.getByRole('option', { name: 'gpt-4o' }));
    expect(screen.getByRole('button', { name: /onboarding\.ai\.next/ })).not.toBeDisabled();

    // Switch from OpenAI (default) to Anthropic.
    await user.click(screen.getByText('Anthropic (Claude)'));

    expect(screen.getByRole('button', { name: /onboarding\.ai\.next/ })).toBeDisabled();
  });
});
