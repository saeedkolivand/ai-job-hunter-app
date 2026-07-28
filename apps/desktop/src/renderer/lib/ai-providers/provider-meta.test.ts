import { describe, expect, it } from 'vitest';

import { CLOUD_DEFAULT_MODELS as aiSelectionDefaults } from '@/features/onboarding/steps/AISelectionStep';
import { CLOUD_DEFAULT_MODELS as cloudProviderPanelDefaults } from '@/features/onboarding/steps/ollama/CloudProviderPanel';
import type { AiProvider } from '@/store/preferences-schema';

import { isOllamaFamily, PROVIDER_ORDER, PROVIDERS } from './provider-meta';

describe('isOllamaFamily', () => {
  it('is true only for the Ollama local + cloud providers', () => {
    expect(isOllamaFamily('ollama')).toBe(true);
    expect(isOllamaFamily('ollama-cloud')).toBe(true);
  });

  it('is false for every non-Ollama provider', () => {
    for (const p of [
      'openai',
      'anthropic',
      'gemini',
      'openai-compatible',
      'claude-code',
    ] as const) {
      expect(isOllamaFamily(p)).toBe(false);
    }
  });
});

describe('Ollama Cloud registration', () => {
  it('is registered as a cloud provider and listed in order', () => {
    expect(PROVIDERS['ollama-cloud'].kind).toBe('cloud');
    expect(PROVIDERS['ollama-cloud'].docsUrl).toContain('ollama.com');
    expect(PROVIDER_ORDER).toContain('ollama-cloud');
  });

  it('keeps PROVIDER_ORDER in sync with PROVIDERS (every entry has meta)', () => {
    for (const p of PROVIDER_ORDER) {
      expect(PROVIDERS[p]).toBeDefined();
    }
  });
});

// Contract test (CodeRabbit, PR #901): both onboarding copies of
// CLOUD_DEFAULT_MODELS must keep pointing at a model that actually exists in
// that provider's curated list — guards against a future model-list refresh
// forgetting to update the onboarding default alongside it.
describe.each([
  ['AISelectionStep', aiSelectionDefaults],
  ['CloudProviderPanel', cloudProviderPanelDefaults],
] as const)('onboarding CLOUD_DEFAULT_MODELS (%s)', (_label, defaults) => {
  it('every default model is in its provider curated list', () => {
    for (const [provider, model] of Object.entries(defaults)) {
      const models = PROVIDERS[provider as AiProvider].models;
      // openai-compatible is BYO (arbitrary server, no curated catalog) — its
      // default is just a placeholder, not a catalog membership claim.
      if (models.length === 0) continue;
      expect(models).toContain(model);
    }
  });
});
