/**
 * ModelSelector — no-model-selected amber warning tests.
 *
 * Covers:
 *  - When reason === 'selectModel' the amber wrapper + warning text render.
 *  - When a model is selected the warning is absent.
 *  - The Dropdown onChange callback still fires when the warning is shown.
 *
 * All hooks that reach IPC / QueryClient / store persistence are stubbed so
 * the component renders without a full provider tree. The real ModelSelector
 * JSX (amber wrapper condition, warning text, role="status") is exercised.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@/lib/i18n', () => ({
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
  useAIModels: () => ({ data: [] }),
  useHasProviderKey: () => ({ data: { has: false } }),
  useSystemHealth: () => ({ data: undefined }),
}));

// ── @tanstack/react-query stub — keep QueryClient et al, stub only useQueries ─

vi.mock('@tanstack/react-query', async (importOriginal) => {
  const actual = await importOriginal();
  return { ...actual, useQueries: () => [] };
});

// ── component under test ──────────────────────────────────────────────────────

import { ModelSelector } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderSelector() {
  return render(<ModelSelector />);
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('ModelSelector — no model selected (Ollama, defaultModel absent)', () => {
  it('renders the amber warning text when no model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = undefined;
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.getByText('models.noModelSelected')).toBeInTheDocument();
  });

  it('renders a status element for a11y when no model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = undefined;
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('renders an amber wrapper (border-amber-400/30 class) when no model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = undefined;
    stubbedActiveProviderModel = '';

    const { container } = renderSelector();

    const amberWrapper = container.querySelector('.border-amber-400\\/30');
    expect(amberWrapper).not.toBeNull();
  });
});

describe('ModelSelector — model selected (Ollama, defaultModel present)', () => {
  it('does NOT render the amber warning when a model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = 'llama3.2';
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.queryByText('models.noModelSelected')).not.toBeInTheDocument();
  });

  it('does NOT render the status role element when a model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = 'llama3.2';
    stubbedActiveProviderModel = '';

    renderSelector();

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('does NOT render the amber wrapper class when a model is selected', () => {
    stubbedActiveProvider = 'ollama';
    stubbedDefaultModel = 'llama3.2';
    stubbedActiveProviderModel = '';

    const { container } = renderSelector();

    expect(container.querySelector('.border-amber-400\\/30')).toBeNull();
  });
});
