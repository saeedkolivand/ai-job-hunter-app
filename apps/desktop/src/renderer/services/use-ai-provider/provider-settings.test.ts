/**
 * `resolveProviderSettingsWrite` — the REPLACE-safety rule.
 *
 * `ai_set_provider_settings` writes every field it is handed, NULL included, so
 * these assert what SURVIVES a save that only meant to change one thing. The
 * window is the field with no second home: dropping it makes a staged run fall
 * back to the provider's default while Settings still shows the slider value.
 */
import { describe, expect, it } from 'vitest';

import { resolveProviderSettingsWrite } from './provider-settings';

describe('resolveProviderSettingsWrite', () => {
  it('keeps the stored window when an unrelated field is saved', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'openai-compatible',
      stored: { model: 'llama3', baseUrl: 'http://old', contextWindow: 16_384 },
      baseUrl: 'http://new',
    });

    expect(write).toEqual({
      provider: 'openai-compatible',
      model: 'llama3',
      baseUrl: 'http://new',
      contextWindow: 16_384,
    });
  });

  it('keeps the stored base URL when only the model is saved', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'openai-compatible',
      stored: { model: 'old-model', baseUrl: 'http://lm-studio' },
      model: 'new-model',
    });

    expect(write.baseUrl).toBe('http://lm-studio');
    expect(write.model).toBe('new-model');
  });

  it('sends the window held for the model being saved', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b' },
      localWindows: { 'qwen3:8b': { contextWindow: 8192 } },
    });

    expect(write.contextWindow).toBe(8192);
  });

  it('does NOT carry the previous model’s window onto a different model', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 32_768 },
      model: 'llama3.2:1b',
      localWindows: { 'qwen3:8b': { contextWindow: 32_768 } },
    });

    expect(write.model).toBe('llama3.2:1b');
    expect(write.contextWindow).toBeUndefined();
  });

  it('prefers an explicitly chosen window (the slider) over the stored limit', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 4096 },
      model: 'qwen3:8b',
      contextWindow: 24_576,
      localWindows: { 'qwen3:8b': { contextWindow: 4096 } },
    });

    expect(write.contextWindow).toBe(24_576);
  });

  it('clears the base URL only when it is explicitly nulled', () => {
    const stored = { model: 'm', baseUrl: 'http://x', contextWindow: 2048 };

    expect(
      resolveProviderSettingsWrite({ provider: 'openai-compatible', stored, baseUrl: null })
    ).toEqual({
      provider: 'openai-compatible',
      model: 'm',
      baseUrl: undefined,
      contextWindow: 2048,
    });
    expect(resolveProviderSettingsWrite({ provider: 'openai-compatible', stored }).baseUrl).toBe(
      'http://x'
    );
  });

  it('writes nothing it was not given for an unseeded provider', () => {
    expect(resolveProviderSettingsWrite({ provider: 'anthropic' })).toEqual({
      provider: 'anthropic',
      model: undefined,
      baseUrl: undefined,
      contextWindow: undefined,
    });
  });
});
