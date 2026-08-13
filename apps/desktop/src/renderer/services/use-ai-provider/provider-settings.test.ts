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

  it('passes an explicit WINDOW clear through, without recomputing it', () => {
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      stored: { model: 'qwen3:8b', contextWindow: 16_384 },
      model: 'llama3.2:1b',
      contextWindow: null,
      // Present, and deliberately NOT used: an explicit clear outranks the
      // model's own stored limit.
      localWindows: { 'llama3.2:1b': { contextWindow: 4096 } },
    });

    expect(write.contextWindow).toBeNull();
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

  it('never CLEARS the window against a row it has not read yet', () => {
    // `stored: undefined` is both "no row" and "the activeConfig query has not
    // resolved". Clearing on that guess wiped real windows.
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      model: 'qwen3:8b',
      localWindows: {},
    });

    expect('contextWindow' in write).toBe(false);
  });

  it('still sends a window the renderer holds for the model, unread row or not', () => {
    // Onboarding / the model picker: nothing is stored yet, but the user has a
    // window for the model they just chose. Sending it destroys nothing.
    const write = resolveProviderSettingsWrite({
      provider: 'ollama',
      model: 'qwen3:8b',
      localWindows: { 'qwen3:8b': { contextWindow: 12_288 } },
    });

    expect(write.contextWindow).toBe(12_288);
  });

  it('sends nothing but the provider when nothing is being changed', () => {
    expect(resolveProviderSettingsWrite({ provider: 'anthropic' })).toEqual({
      provider: 'anthropic',
    });
  });
});
