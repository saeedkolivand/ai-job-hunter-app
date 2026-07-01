import { describe, expect, it } from 'vitest';

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
