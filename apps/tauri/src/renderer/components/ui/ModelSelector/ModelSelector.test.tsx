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

// ── Preferences store stub — control selectedValue via aiModel / providerConfig

// useAIModel / useAiProviderConfig / usePreferencesStore are pulled individually.
// We vi.mock the whole module and expose controllable refs so each test can set
// them before rendering.

let stubbedDefaultModel: string | undefined = undefined;
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

vi.mock('@/store/preferences-store', () => ({
  useAIModel: () =>
    stubbedDefaultModel !== undefined ? { defaultModel: stubbedDefaultModel } : undefined,
  useAiProviderConfig: () => ({
    activeProvider: stubbedActiveProvider,
    providers: {
      [stubbedActiveProvider]: { model: stubbedActiveProviderModel },
    },
  }),
  usePreferencesStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      setAIModel: vi.fn(),
      setProviderSettings: vi.fn(),
      setActiveProvider: vi.fn(),
    }),
}));

// ── Service stubs — prevent QueryClient dependency ────────────────────────────

vi.mock('@/services', () => ({
  useAIModels: () => ({ data: stubbedOllamaModels, isLoading: false }),
  useHasProviderKey: () => ({ data: { has: false } }),
  useSystemHealth: () => stubbedHealth,
}));

// ── @tanstack/react-query stub — keep QueryClient et al, stub only useQueries ─

vi.mock('@tanstack/react-query', async (importOriginal) => {
  // Type the original module as a plain object record so the spread is valid
  // (TS2698) — the only member the component uses, `useQueries`, is overridden
  // below, so the exact shape is irrelevant. A generic (not an inline `import()`
  // type, nor a restricted `@tanstack/react-query` import) keeps ESLint happy.
  const actual = await importOriginal<Record<string, unknown>>();
  return { ...actual, useQueries: () => [] };
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
  stubbedDefaultModel = undefined;
  stubbedActiveProviderModel = '';
  stubbedOllamaModels = [];
  stubbedHealth = { data: undefined, isLoading: false };
});

describe('ModelSelector — no model selected (Ollama, defaultModel absent)', () => {
  it('renders the amber warning text when no model is selected', () => {
    stubbedDefaultModel = undefined;

    renderSelector();

    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });

  it('renders a status element for a11y when no model is selected', () => {
    stubbedDefaultModel = undefined;

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('renders an amber wrapper (border-amber-400/30 class) when no model is selected', () => {
    stubbedDefaultModel = undefined;

    const { container } = renderSelector();

    const amberWrapper = container.querySelector('.border-amber-400\\/30');
    expect(amberWrapper).not.toBeNull();
  });
});

describe('ModelSelector — model selected and visible (Ollama, defaultModel in available models)', () => {
  it('does NOT render the amber warning when the selected model is a visible option', () => {
    stubbedDefaultModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });

  it('does NOT render the status role element when the selected model is a visible option', () => {
    stubbedDefaultModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('does NOT render the amber wrapper class when the selected model is a visible option', () => {
    stubbedDefaultModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'llama3.2' }];

    const { container } = renderSelector();

    expect(container.querySelector('.border-amber-400\\/30')).toBeNull();
  });
});

describe('ModelSelector — model selected but unavailable (Ollama, defaultModel not in available models)', () => {
  it('renders the modelUnavailable warning (not noModelSelected) when the list is empty', () => {
    stubbedDefaultModel = 'llama3.2';
    stubbedOllamaModels = [];

    renderSelector();

    const warning = screen.getByRole('status');
    expect(warning).toHaveTextContent('models.modelUnavailable');
    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
  });

  it('renders the modelUnavailable warning when the list contains only other models', () => {
    stubbedDefaultModel = 'llama3.2';
    stubbedOllamaModels = [{ name: 'qwen2.5' }];

    renderSelector();

    expect(screen.getByText('models.modelUnavailable')).toBeInTheDocument();
  });

  it('renders the amber wrapper for an unavailable selected model', () => {
    stubbedDefaultModel = 'llama3.2';
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
