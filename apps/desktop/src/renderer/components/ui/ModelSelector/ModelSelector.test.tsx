/**
 * ModelSelector — model-warning tests.
 *
 * The warning is provider-agnostic: it fires for EVERY provider kind whenever the
 * dropdown isn't showing a visibly-selected model (`!modelsLoading &&
 * !selectedModelVisible`), suppressed only while the active provider's option
 * source is loading.
 *
 * Covers:
 *  - No model value (Ollama) → amber wrapper + `models.noModelSelected`.
 *  - Stored Ollama model IS a visible dropdown option → warning absent.
 *  - Stored model NOT a visible option (uninstalled / stale) → `models.modelUnavailable`.
 *  - A detected CLI agent with no model picked → warning RENDERS (CLI is no longer
 *    exempt — a model must always be visibly selected).
 *  - A detected CLI agent with one of its curated models selected → warning absent.
 *  - Cloud model-list health for the ACTIVE provider (live-model-lists PR — the
 *    curated cloud fallback is gone; catalogues are fetched live with a
 *    last-good local cache):
 *      - Not connected (no stored key) → a neutral "add a key" note, not an error.
 *      - Live fetch failed with a cached list available → the dropdown still
 *        lists the cached models, plus a "showing cached list" note.
 *      - Live fetch failed with NO cache → the real failure message.
 *
 * All hooks that reach IPC / QueryClient / store persistence are stubbed so
 * the component renders without a full provider tree. The real ModelSelector
 * JSX (amber wrapper condition, warning text, role="status") is exercised.
 * `stubbedOllamaModels` drives Ollama `options` and `stubbedHealth` drives CLI-agent
 * `options` + the cli-agent `modelsLoading` branch, so a test makes the stored
 * selection a visible option (no warning) or absent (warning).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── AppClientProvider stub ────────────────────────────────────────────────────

vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({
    ai: {
      hasProviderKey: vi.fn().mockResolvedValue({ has: false }),
      listProviderModels: vi.fn().mockResolvedValue([]),
    },
  }),
}));

// ── Active-config stub — control selectedValue via the backend routing query ───

// Routing (activeProvider + per-provider model) is backend-owned (task #16), read
// via `useActiveConfig`; the model now lives in `providers[activeProvider].model`
// for EVERY provider kind (the old `aiModel` Ollama mirror is gone).
let stubbedActiveProvider = 'ollama';
let stubbedActiveProviderModel = '';
// Installed Ollama model names — drives `options` via buildModelOptions, so a test
// can make the stored selection a visible option (warning suppressed) or not
// (warning shown). Mirrors the other module-level stub refs above.
let stubbedOllamaModels: Array<{ name: string }> = [];
// System-health probe result — drives CLI-agent option availability (a CLI agent
// contributes its curated models only when `detected`) and the cli-agent branch of
// `modelsLoading` (via `isLoading`). Controllable so a test can mark a CLI agent
// detected with the probe settled.
let stubbedHealth: {
  data: { cliAgents?: Record<string, { detected: boolean }> } | undefined;
  isLoading: boolean;
} = { data: undefined, isLoading: false };

// ── Service stubs — prevent QueryClient dependency ────────────────────────────

vi.mock('@/services', () => ({
  useActiveConfig: () => ({
    data: {
      activeProvider: stubbedActiveProvider,
      model: stubbedActiveProviderModel,
      providers: { [stubbedActiveProvider]: { model: stubbedActiveProviderModel } },
    },
    isPending: false,
  }),
  useConfigureActiveProvider: () => ({ mutate: vi.fn() }),
  useAIModels: () => ({ data: stubbedOllamaModels, isLoading: false }),
  useHasProviderKey: () => ({ data: { has: false } }),
  useSystemHealth: () => stubbedHealth,
}));

// ── @tanstack/react-query stub — keep QueryClient et al, stub only useQueries ──
//
// ModelSelector fires two `useQueries` calls (key-status + model-list), each
// keyed `[...keys.ai.models, 'provider-key' | 'provider-models', provider, ...]`
// — a per-test fixture keyed by that same `[kind, provider]` pair lets a test
// express a cloud provider as connected (`provider-key`) while its
// `provider-models` query is settled in a particular state (fresh / cached /
// errored), so the picker's cache/error handling (live-model-lists PR) is
// exercised through the real component rather than stubbed away.
type CloudQueryState = { data?: unknown; isLoading?: boolean; isError?: boolean; error?: unknown };
let stubbedCloudKeyQueries: Record<string, CloudQueryState> = {};
let stubbedCloudModelQueries: Record<string, CloudQueryState> = {};

vi.mock('@tanstack/react-query', async (importOriginal) => {
  // Type the original module as a plain object record so the spread is valid
  // (TS2698) — the only member the component uses, `useQueries`, is overridden
  // below, so the exact shape is irrelevant. A generic (not an inline `import()`
  // type, nor a restricted `@tanstack/react-query` import) keeps ESLint happy.
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    useQueries: ({ queries }: { queries: Array<{ queryKey: unknown[] }> }) =>
      queries.map(({ queryKey }) => {
        const [, , kind, provider] = queryKey as [unknown, unknown, string, string];
        const table = kind === 'provider-key' ? stubbedCloudKeyQueries : stubbedCloudModelQueries;
        return (
          table[provider] ?? {
            data: kind === 'provider-key' ? { has: false } : { models: [], cached: false },
            isLoading: false,
          }
        );
      }),
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';

import { ModelSelector } from './index';

// A real cli-agent provider id from the registry (e.g. claude-code) — derived so
// the test tracks the registry rather than hardcoding an id that may be renamed.
const cliAgentId = PROVIDER_ORDER.find((p) => PROVIDERS[p].kind === 'cli-agent');
if (!cliAgentId) throw new Error('expected at least one cli-agent provider in the registry');
const cliAgentModel = PROVIDERS[cliAgentId].models[0];
if (!cliAgentModel) throw new Error(`expected ${cliAgentId} to expose a curated model`);

// A real cloud provider id, likewise derived from the registry rather than
// hardcoded. Cloud providers carry no curated `models` list any more
// (live-model-lists PR) — every model in these tests comes from a stubbed
// `provider-models` query result, never from the registry.
const cloudProviderId = PROVIDER_ORDER.find((p) => PROVIDERS[p].kind === 'cloud');
if (!cloudProviderId) throw new Error('expected at least one cloud provider in the registry');

// ── helpers ───────────────────────────────────────────────────────────────────

function renderSelector() {
  return render(<ModelSelector />);
}

// ── tests ─────────────────────────────────────────────────────────────────────

// Reset every controllable stub to a deterministic baseline. `stubbedOllamaModels`
// is shared and the warning now couples to the model list, so leakage between
// tests would otherwise flip the warning condition.
beforeEach(() => {
  stubbedActiveProvider = 'ollama';
  stubbedActiveProviderModel = '';
  stubbedOllamaModels = [];
  stubbedHealth = { data: undefined, isLoading: false };
  stubbedCloudKeyQueries = {};
  stubbedCloudModelQueries = {};
});

describe('ModelSelector — no model selected (Ollama, model absent)', () => {
  it('renders the amber warning text when no model is selected', () => {
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });

  it('renders a status element for a11y when no model is selected', () => {
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('renders an amber wrapper (border-amber-400/30 class) when no model is selected', () => {
    stubbedActiveProviderModel = '';

    const { container } = renderSelector();

    const amberWrapper = container.querySelector('.border-amber-400\\/30');
    expect(amberWrapper).not.toBeNull();
  });
});

describe('ModelSelector — model selected and visible (Ollama, model in available models)', () => {
  it('does NOT render the amber warning when the selected model is a visible option', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });

  it('does NOT render the status role element when the selected model is a visible option', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('does NOT render the amber wrapper class when the selected model is a visible option', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    const { container } = renderSelector();

    expect(container.querySelector('.border-amber-400\\/30')).toBeNull();
  });
});

describe('ModelSelector — model selected but unavailable (Ollama, model not in available models)', () => {
  it('renders the modelUnavailable warning (not noModelSelected) when the list is empty', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [];

    renderSelector();

    const warning = screen.getByRole('status');
    expect(warning).toHaveTextContent('models.modelUnavailable');
    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
  });

  it('renders the modelUnavailable warning when the list contains only other models', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'qwen2.5' }];

    renderSelector();

    expect(screen.getByText('models.modelUnavailable')).toBeInTheDocument();
  });

  it('renders the amber wrapper for an unavailable selected model', () => {
    stubbedActiveProviderModel = 'llama3.2';
    stubbedOllamaModels = [];

    const { container } = renderSelector();

    expect(container.querySelector('.border-amber-400\\/30')).not.toBeNull();
  });
});

describe('ModelSelector — CLI agent detected, no model selected', () => {
  it('renders the amber warning for a detected CLI agent with no model picked', () => {
    // Detected CLI agent (probe settled) but no stored model → selectedValue is ''.
    // Its curated models build options (e.g. `claude-code||sonnet`) that never match
    // the empty selection, so the placeholder/warning path is exercised for a CLI
    // provider — the warning must fire for every provider kind, CLI included.
    stubbedActiveProvider = cliAgentId;
    stubbedActiveProviderModel = '';
    stubbedHealth = { data: { cliAgents: { [cliAgentId]: { detected: true } } }, isLoading: false };

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });
});

describe('ModelSelector — CLI agent detected, model selected and visible', () => {
  it('does NOT render the warning when a detected CLI agent has one of its models selected', () => {
    // selectedValue is `cliAgentId||cliAgentModel`, which IS a visible option built
    // from the agent's curated models, so no warning shows.
    stubbedActiveProvider = cliAgentId;
    stubbedActiveProviderModel = cliAgentModel;
    stubbedHealth = { data: { cliAgents: { [cliAgentId]: { detected: true } } }, isLoading: false };

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — active cloud provider, no key stored', () => {
  it('shows the neutral "add a key" note as a polite status region, not an error', () => {
    // No curated fallback exists any more (live-model-lists PR removed it) —
    // an un-keyed cloud provider must read as "add a key", never as a failure.
    stubbedActiveProvider = cloudProviderId;
    stubbedActiveProviderModel = '';
    stubbedCloudKeyQueries = { [cloudProviderId]: { data: { has: false }, isLoading: false } };

    renderSelector();

    const note = screen.getByText('models.cloud.addKeyToLoad');
    expect(note).toBeInTheDocument();
    expect(note).toHaveAttribute('role', 'status');
    expect(note).toHaveAttribute('aria-live', 'polite');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
    expect(screen.queryByText('settings.aiModel.loading')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — connected cloud provider, model list still loading', () => {
  it('shows a polite loading status — the state none of the other three hints cover', () => {
    stubbedActiveProvider = cloudProviderId;
    stubbedActiveProviderModel = '';
    stubbedCloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    stubbedCloudModelQueries = {
      [cloudProviderId]: { data: undefined, isLoading: true, isError: false },
    };

    renderSelector();

    const loading = screen.getByText('settings.aiModel.loading');
    expect(loading).toBeInTheDocument();
    expect(loading.closest('[role="status"]')).not.toBeNull();
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — connected cloud provider, live fetch failed, cache available', () => {
  it('lists the cached models and notes the list is cached, as a polite status (not an error)', async () => {
    const cachedModel = 'cached-model-x';
    stubbedActiveProvider = cloudProviderId;
    stubbedActiveProviderModel = cachedModel;
    stubbedCloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    // The fetch-with-cache wrapper RESOLVES (not an error) when a cached list
    // is available — only rejects when both the live fetch AND the cache miss.
    stubbedCloudModelQueries = {
      [cloudProviderId]: {
        data: { models: [{ name: cachedModel }], cached: true },
        isLoading: false,
        isError: false,
      },
    };

    renderSelector();

    const note = screen.getByText('models.cloud.cachedList');
    expect(note).toBeInTheDocument();
    expect(note).toHaveAttribute('role', 'status');
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('option', { name: cachedModel })).toBeInTheDocument();
  });
});

describe('ModelSelector — connected cloud provider, live fetch failed, no cache', () => {
  it('shows the real failure message as role="alert"', () => {
    stubbedActiveProvider = cloudProviderId;
    stubbedActiveProviderModel = '';
    stubbedCloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    stubbedCloudModelQueries = {
      [cloudProviderId]: {
        data: undefined,
        isLoading: false,
        isError: true,
        error: new Error('invalid or unauthorized API key'),
      },
    };

    renderSelector();

    // `t` is stubbed to the identity function and drops interpolation params,
    // so the key itself is what renders — the real message reaches the DOM
    // via the component's own `t(key, { message })` call, which is the
    // behaviour under test (PR 1's classified error finally has a reader).
    const failure = screen.getByText('models.cloud.fetchFailed');
    expect(failure).toBeInTheDocument();
    expect(failure.closest('[role="alert"]')).not.toBeNull();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
  });
});
