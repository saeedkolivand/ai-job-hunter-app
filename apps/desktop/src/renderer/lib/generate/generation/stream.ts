import { getClient } from '../../app-client';
import { safeLocale } from '../locales';
import {
  type GenerationIntent,
  resolveActiveProvider,
  resolveTemperatureOverride,
  type TemperatureStep,
} from '../provider-context';
import { awaitAiStream } from '../stream-promise';

/** Per-call transport options shared by every generation surface. */
export interface StreamOptions {
  onToken?: (tok: string) => void;
  /** Transport locale; clamped to a supported one via `safeLocale`. Default 'en'. */
  locale?: string;
  signal?: AbortSignal;
  onThinking?: (tok: string) => void;
}

/**
 * Stream one generation step through the backend orchestration pipeline
 * (`ai.generatePipeline`, `ai:stream` deltas under the returned jobId).
 *
 * Sampling: `step` selects the user's per-model temperature OVERRIDE (Ollama-only,
 * see {@link resolveTemperatureOverride}); `intent` is the caller's declared, fixed
 * classification for the surface (never computed from tier/model). NEITHER carries
 * a raw default number — each provider adapter picks its own numbers (or none) per
 * `(model, intent)` via `AiProvider::sampling_profile`
 * (`commands/ai_provider/mod.rs`), the renderer states intent only.
 */
export async function streamGenerate(
  model: string,
  system: string,
  user: string,
  step: TemperatureStep,
  intent: GenerationIntent,
  { onToken, locale = 'en', signal, onThinking }: StreamOptions = {}
): Promise<string> {
  const temperature = resolveTemperatureOverride(step);
  const api = getClient();
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider(model);
  // Per-model generation limits are local (Ollama) only — cloud/CLI providers
  // ignore them, and the backend only applies num_predict/num_ctx for Ollama.
  const localLimits =
    activeProvider === 'ollama' ? providerSettings?.modelLimits?.[activeModel] : undefined;
  const res = await api.ai.generatePipeline({
    model: activeModel,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    locale: safeLocale(locale),
    // The Ollama per-model override ONLY — an explicit user choice that wins on
    // every adapter. No other sampling knob is sent from here.
    temperature,
    intent,
    // provider + baseUrl are NOT sent (task #16): the backend resolves the active
    // provider/base_url from its own store and overwrites `model` before streaming,
    // so an XSS'd renderer can no longer point generation at an arbitrary endpoint.
    // `effort` (a tuning knob every reasoning-capable provider reads, not routing) stays.
    effort: providerSettings?.effort,
    // Per-model local limits (Ollama) — omitted for cloud/CLI or when unset.
    maxTokens: localLimits?.maxTokens,
    contextWindow: localLimits?.contextWindow,
  });

  return awaitAiStream(api, res.jobId, {
    onToken,
    onThinking,
    signal,
    provider: activeProvider,
    model: activeModel,
    // Same value just sent to the backend — sizes the renderer-side timeout so a
    // high-effort generation isn't killed client-side while the backend is still
    // legitimately streaming (`computeStreamTimeoutMs`).
    effort: providerSettings?.effort,
  });
}
