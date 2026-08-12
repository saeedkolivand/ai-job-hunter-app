/**
 * `resolveProviderSettingsWrite` — what a one-field save actually sends.
 *
 * `ai_set_provider_settings` is PATCH per field (omitted = keep, `null` =
 * clear), so these assert two things: that the writer sends only what changed,
 * and that a MODEL change never leaves the previous model's context window
 * attached to it — the one case where "send nothing" is wrong.
 */
import { describe, expect, it } from 'vitest';

import { resolveProviderSettingsWrite } from './provider-settings';

describe('resolveProviderSettingsWrite', () => {
  it('sends only the field being changed', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'openai-compatible',
      stored: { model: 'llama3', baseUrl: 'http://old', contextWindow: 16_384 },
      baseUrl: 'http://new',
    });

    expect(write).toEqual({ provider: 'openai-compatible', baseUrl: 'http://new' });
    // The stored model and window are untouched because they are not sent.
    expect('model' in write).toBe(false);
    expect('contextWindow' in write).toBe(false);
  });

  it('re-points the window at the model being saved', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 32_768 },
      model: 'llama3.2:1b',
      localWindows: { 'llama3.2:1b': { contextWindow: 4096 } },
    });

    expect(write).toEqual({ provider: 'ollama', model: 'llama3.2:1b', contextWindow: 4096 });
  });

  it('CLEARS the window when the new model has none of its own', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 32_768 },
      model: 'llama3.2:1b',
      localWindows: { 'qwen3:8b': { contextWindow: 32_768 } },
    });

    // Explicit null, not omission: omitting would silently run the new model at
    // the old model's num_ctx.
    expect(write.contextWindow).toBeNull();
  });

  it('leaves the window alone when the model is re-saved unchanged', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 32_768 },
      model: 'qwen3:8b',
    });

    expect('contextWindow' in write).toBe(false);
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

  it('passes an explicit clear straight through', () => {
    expect(
      resolveProviderSettingsWrite({
        provider: 'openai-compatible',
        stored: { model: 'm', baseUrl: 'http://x', contextWindow: 2048 },
        baseUrl: null,
      })
    ).toEqual({ provider: 'openai-compatible', baseUrl: null });
  });

  it('sends nothing but the provider when nothing is being changed', () => {
    expect(resolveProviderSettingsWrite({ provider: 'anthropic' })).toEqual({
      provider: 'anthropic',
    });
  });
});
