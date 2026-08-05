/**
 * CloudProviderPanel — onboarding cloud-provider tests (live-model-lists PR).
 *
 * Model choice is deferred until a key is stored: no id is ever pre-selected
 * (the defect class this step now avoids — a shut-down `gemini-2.0-flash`
 * shipped as the hardcoded Gemini onboarding default). Once a key is stored,
 * `useListProviderModels` (the SAME cache-aware hook the picker and Settings
 * use) drives the model section through the states it already defines:
 * loading, the real failure message (no cache), a cached-list note, and a
 * genuinely-empty catalogue — plus the live/fresh list rendered as a real
 * `Dropdown`. Every state asserts its OWN positive affordance (element +
 * role), not just the absence of a wrong one — a loading state that renders
 * nothing (falling through to a bare empty `<Dropdown>`) would otherwise pass
 * an absence-only assertion silently.
 *
 * Also covers: focus moves to the "Choose a model" heading on the actual
 * false→true `hasKey` transition (never on mount with a key already stored —
 * that would steal focus from wherever the user was on step entry).
 *
 * `@/services` is mocked directly (no QueryClient/AppClientProvider needed —
 * every hook the component calls is a service hook, mirroring AdzunaKeyStep's
 * test strategy).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub — key passthrough, drops interpolation params (mirrors
// ModelSelector.test.tsx: the key itself is what renders and is asserted). ──

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @/services stub ──────────────────────────────────────────────────────────

let stubHasKey = false;
type ModelsQueryState = {
  data?: { models: Array<{ name: string; displayName?: string }>; cached: boolean };
  isLoading: boolean;
  isError: boolean;
  error?: unknown;
};
let stubModelsQuery: ModelsQueryState = { data: undefined, isLoading: false, isError: false };

const setProviderKeyMutateAsync = vi.fn().mockResolvedValue(undefined);
const testProviderKeyMutateAsync = vi.fn().mockResolvedValue({ success: true });
const notifySuccessSpy = vi.fn();
const notifyErrorSpy = vi.fn();

vi.mock('@/services', () => ({
  useHasProviderKey: () => ({ data: { has: stubHasKey } }),
  useListProviderModels: () => stubModelsQuery,
  useOpenExternal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useSetProviderKey: () => ({ mutateAsync: setProviderKeyMutateAsync }),
  useTestProviderKey: () => ({ mutateAsync: testProviderKeyMutateAsync }),
}));

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    useNotification: () => ({ success: notifySuccessSpy, error: notifyErrorSpy }),
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { CloudProviderPanel } from './index';

const SELECT_PLACEHOLDER = 'onboarding.ai.selectModelPlaceholder';

function renderPanel(overrides: Partial<Parameters<typeof CloudProviderPanel>[0]> = {}) {
  const onProviderChange = vi.fn();
  const onModelSelect = vi.fn();
  const props = {
    selectedProvider: 'openai' as const,
    onProviderChange,
    selectedModel: '',
    onModelSelect,
    ...overrides,
  };
  const result = render(<CloudProviderPanel {...props} />);
  return { ...result, onProviderChange, onModelSelect };
}

beforeEach(() => {
  stubHasKey = false;
  stubModelsQuery = { data: undefined, isLoading: false, isError: false };
  setProviderKeyMutateAsync.mockClear();
  testProviderKeyMutateAsync.mockClear();
  notifySuccessSpy.mockClear();
  notifyErrorSpy.mockClear();
});

describe('CloudProviderPanel — no key stored', () => {
  it('shows the API key input, not the model picker', () => {
    stubHasKey = false;
    renderPanel();

    expect(screen.getByPlaceholderText('sk-...')).toBeInTheDocument();
    expect(screen.queryByText('onboarding.ai.chooseModel')).not.toBeInTheDocument();
  });

  it('saving a key does NOT pre-select or configure a model on the hasKey transition (no hardcoded default)', async () => {
    stubHasKey = false;
    // A non-empty list, so a default-selection effect firing on the
    // false→true transition would have something to (wrongly) pick.
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    const { onModelSelect, rerender } = renderPanel();

    await user.type(screen.getByPlaceholderText('sk-...'), 'sk-test-key');
    await user.click(screen.getByRole('button', { name: 'onboarding.ai.saveKey' }));

    expect(setProviderKeyMutateAsync).toHaveBeenCalledWith({
      provider: 'openai',
      apiKey: 'sk-test-key',
    });

    // Exercise the ACTUAL false→true transition — leaving `stubHasKey` false
    // throughout (the earlier version of this test) means the component
    // never reaches the model-picker tree at all, so it could not have
    // caught a default-selection effect firing on that transition.
    stubHasKey = true;
    rerender(
      <CloudProviderPanel
        selectedProvider="openai"
        onProviderChange={vi.fn()}
        selectedModel=""
        onModelSelect={onModelSelect}
      />
    );

    expect(onModelSelect).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: SELECT_PLACEHOLDER })).toBeInTheDocument();
  });
});

describe('CloudProviderPanel — key stored, model list loading', () => {
  it('shows a polite loading status — not an error and not a bare empty dropdown', () => {
    stubHasKey = true;
    stubModelsQuery = { data: undefined, isLoading: true, isError: false };
    renderPanel();

    const loading = screen.getByText('settings.aiModel.loading');
    expect(loading.closest('[role="status"]')).not.toBeNull();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: SELECT_PLACEHOLDER })).not.toBeInTheDocument();
  });
});

describe('CloudProviderPanel — key stored, live fetch failed, no cache', () => {
  it('shows the real failure message as an alert', () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('invalid or unauthorized API key'),
    };
    renderPanel();

    // `Alert` carries role="alert" itself — the always-present "key stored"
    // success Alert is ALSO role="alert" (a separate, pre-existing concern:
    // `Alert` sets it unconditionally, not just for `type="error"`), so
    // assert by content rather than assuming a single alert in the tree.
    const failure = screen.getByText('models.cloud.fetchFailed');
    expect(failure.closest('[role="alert"]')).not.toBeNull();
    expect(screen.queryByRole('button', { name: SELECT_PLACEHOLDER })).not.toBeInTheDocument();
  });
});

describe('CloudProviderPanel — key stored, fetch succeeded but the catalogue is empty', () => {
  it('shows the neutral empty title AND description as a polite status (not an amber warning)', () => {
    stubHasKey = true;
    stubModelsQuery = { data: { models: [], cached: false }, isLoading: false, isError: false };
    renderPanel();

    const title = screen.getByText('settings.aiModel.emptyTitle');
    expect(title.closest('[role="status"]')).not.toBeNull();
    expect(screen.getByText('settings.aiModel.emptyDescription')).toBeInTheDocument();
    // Not itself an alert — the always-present "key stored" success Alert
    // (role="alert" regardless of type — a separate, pre-existing concern)
    // means a document-wide alert query isn't the right assertion here.
    expect(title.closest('[role="alert"]')).toBeNull();
  });
});

describe('CloudProviderPanel — key stored, live fetch failed, cache available', () => {
  it('lists the cached models and shows the cached-list note as a polite status (not an error)', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'cached-model' }], cached: true },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    renderPanel();

    const note = screen.getByText('models.cloud.cachedList');
    expect(note.closest('[role="status"]')).not.toBeNull();
    expect(note.closest('[role="alert"]')).toBeNull();

    await user.click(screen.getByRole('button', { name: SELECT_PLACEHOLDER }));
    expect(screen.getByRole('option', { name: 'cached-model' })).toBeInTheDocument();
  });
});

describe('CloudProviderPanel — key stored, live fetch succeeded', () => {
  it('lists every live model and calls onModelSelect when one is picked', async () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: {
        models: [{ name: 'gpt-4o', displayName: 'GPT-4o' }, { name: 'o1' }],
        cached: false,
      },
      isLoading: false,
      isError: false,
    };
    const user = userEvent.setup();
    const { onModelSelect } = renderPanel();

    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: SELECT_PLACEHOLDER }));
    expect(screen.getByRole('option', { name: 'GPT-4o' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'o1' })).toBeInTheDocument();

    await user.click(screen.getByRole('option', { name: 'GPT-4o' }));
    expect(onModelSelect).toHaveBeenCalledWith('gpt-4o');
  });

  it('shows the currently selected model as the dropdown value', () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o', displayName: 'GPT-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    renderPanel({ selectedModel: 'gpt-4o' });

    expect(screen.getByRole('button', { name: /GPT-4o/ })).toBeInTheDocument();
  });
});

describe('CloudProviderPanel — focus management on the hasKey transition', () => {
  it('moves focus to the "Choose a model" heading right after a key is newly stored', () => {
    stubHasKey = false;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    const { rerender } = renderPanel();

    expect(screen.queryByText('onboarding.ai.chooseModel')).not.toBeInTheDocument();

    stubHasKey = true;
    rerender(
      <CloudProviderPanel
        selectedProvider="openai"
        onProviderChange={vi.fn()}
        selectedModel=""
        onModelSelect={vi.fn()}
      />
    );

    expect(screen.getByText('onboarding.ai.chooseModel')).toHaveFocus();
  });

  it('does NOT steal focus on mount when a key is already stored', () => {
    stubHasKey = true;
    stubModelsQuery = {
      data: { models: [{ name: 'gpt-4o' }], cached: false },
      isLoading: false,
      isError: false,
    };
    renderPanel();

    expect(screen.getByText('onboarding.ai.chooseModel')).not.toHaveFocus();
  });
});
