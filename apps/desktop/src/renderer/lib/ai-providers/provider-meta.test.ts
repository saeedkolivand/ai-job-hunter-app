import { describe, expect, it } from 'vitest';

import { isOllamaFamily, isProviderConfigured, PROVIDER_ORDER, PROVIDERS } from './provider-meta';

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

  it('cloud providers carry no curated model list — catalogues are fetched live, not hand-maintained', () => {
    for (const p of PROVIDER_ORDER) {
      if (PROVIDERS[p].kind === 'cloud') expect(PROVIDERS[p].models).toEqual([]);
    }
  });
});

describe('isProviderConfigured', () => {
  it('is true for any provider with a stored key, regardless of base URL', () => {
    expect(isProviderConfigured('openai', true)).toBe(true);
    expect(isProviderConfigured('openai-compatible', true, undefined)).toBe(true);
  });

  it('is false for a non-openai-compatible provider with no stored key, even with a base URL', () => {
    // The `openai-compatible` keyless carve-out must not leak to any other
    // provider — every other cloud provider still requires a key, full stop.
    expect(isProviderConfigured('openai', false, 'https://x')).toBe(false);
  });

  it('is true for openai-compatible with a non-blank base URL and no key (the #936 keyless case)', () => {
    expect(isProviderConfigured('openai-compatible', false, 'http://localhost:1234/v1')).toBe(true);
  });

  it('is false for openai-compatible with neither a key nor a base URL (privacy regression)', () => {
    // The exact bug fixed on fix/no-unconfigured-openai-probe: an unconfigured
    // openai-compatible provider must never read as usable — it falls back to
    // api.openai.com server-side with no requirement to be pointed anywhere.
    expect(isProviderConfigured('openai-compatible', false, undefined)).toBe(false);
    expect(isProviderConfigured('openai-compatible', false, '')).toBe(false);
    expect(isProviderConfigured('openai-compatible', false, '   ')).toBe(false);
  });
});
