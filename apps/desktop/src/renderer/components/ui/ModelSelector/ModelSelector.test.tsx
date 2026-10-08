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
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';

import { ModelSelector } from './index';
import { resetStub, stub } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-support')).translationsMock());
vi.mock('@/providers/AppClientProvider', async () =>
  (await import('./test-support')).appClientMock()
);
vi.mock('@/services', async () => (await import('./test-support')).servicesMock());
vi.mock('@tanstack/react-query', async (importOriginal) =>
  (await import('./test-support')).reactQueryMock(await importOriginal())
);

// A real cli-agent provider id from the registry (e.g. claude-code) — derived so
// the test tracks the registry rather than hardcoding an id that may be renamed.
const cliAgentId = PROVIDER_ORDER.find((p) => PROVIDERS[p].kind === 'cli-agent');
if (!cliAgentId) throw new Error('expected at least one cli-agent provider in the registry');
const cliAgentModel = PROVIDERS[cliAgentId].models[0];
if (!cliAgentModel) throw new Error(`expected ${cliAgentId} to expose a curated model`);

const renderSelector = () => render(<ModelSelector />);

// `stub` is shared and the warning couples to the model list, so leakage between
// tests would otherwise flip the warning condition.
beforeEach(resetStub);

describe('ModelSelector — no model selected (Ollama, model absent)', () => {
  it('renders the amber warning text when no model is selected', () => {
    stub.activeProviderModel = '';

    renderSelector();

    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });

  it('renders a status element for a11y when no model is selected', () => {
    stub.activeProviderModel = '';

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('renders an amber wrapper (border-amber-400/30 class) when no model is selected', () => {
    stub.activeProviderModel = '';

    const { container } = renderSelector();

    const amberWrapper = container.querySelector('.border-amber-400\\/30');
    expect(amberWrapper).not.toBeNull();
  });
});

describe('ModelSelector — embedding-only models', () => {
  it('are not offered as chat models', () => {
    stub.activeProviderModel = 'qwen3-embedding:4b';
    stub.ollamaModels = [{ name: 'nomic-embed-text' }, { name: 'llama3.2' }];

    renderSelector();

    // The stored model is not in the list at all (and is not the filtered one).
    expect(screen.getByText('models.modelUnavailable')).toBeInTheDocument();
  });

  it('keeps a SAVED embedding-only model visible as the current value', () => {
    stub.activeProviderModel = 'nomic-embed-text';
    stub.ollamaModels = [{ name: 'nomic-embed-text' }, { name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — model selected and visible (Ollama, model in available models)', () => {
  it('does NOT render the amber warning when the selected model is a visible option', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });

  it('does NOT render the status role element when the selected model is a visible option', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [{ name: 'llama3.2' }];

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('does NOT render the amber wrapper class when the selected model is a visible option', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [{ name: 'llama3.2' }];

    const { container } = renderSelector();

    expect(container.querySelector('.border-amber-400\\/30')).toBeNull();
  });
});

describe('ModelSelector — model selected but unavailable (Ollama, model not in available models)', () => {
  it('renders the modelUnavailable warning (not noModelSelected) when the list is empty', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [];

    renderSelector();

    const warning = screen.getByRole('status');
    expect(warning).toHaveTextContent('models.modelUnavailable');
    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
  });

  it('renders the modelUnavailable warning when the list contains only other models', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [{ name: 'qwen2.5' }];

    renderSelector();

    expect(screen.getByText('models.modelUnavailable')).toBeInTheDocument();
  });

  it('renders the amber wrapper for an unavailable selected model', () => {
    stub.activeProviderModel = 'llama3.2';
    stub.ollamaModels = [];

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
    stub.activeProvider = cliAgentId;
    stub.activeProviderModel = '';
    stub.health = { data: { cliAgents: { [cliAgentId]: { detected: true } } }, isLoading: false };

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });
});

describe('ModelSelector — CLI agent detected, model selected and visible', () => {
  it('does NOT render the warning when a detected CLI agent has one of its models selected', () => {
    // selectedValue is `cliAgentId||cliAgentModel`, which IS a visible option built
    // from the agent's curated models, so no warning shows.
    stub.activeProvider = cliAgentId;
    stub.activeProviderModel = cliAgentModel;
    stub.health = { data: { cliAgents: { [cliAgentId]: { detected: true } } }, isLoading: false };

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
    expect(screen.queryByText('models.modelUnavailable')).not.toBeInTheDocument();
  });
});
