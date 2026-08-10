/**
 * Provider-context helpers shared across generation modules.
 *
 * Centralises the three repeated concerns that previously appeared in every
 * exported function in `generation.ts` and `resume-ai.ts`:
 *
 *  1. `resolveActiveProvider` — reads the active provider + settings from the
 *     preferences store and returns the resolved model name.
 *  2. `resolveEffectiveTier` — single source-of-truth for the `ModelTier` that
 *     drives prompt verbosity, replacing two private copies that diverged (the
 *     `resume-ai.ts` copy ignored its `provider` parameter, causing compact-mode
 *     to wrongly return `'large'` for cloud providers).
 *  3. `buildProviderProfile` — constructs a `ProviderProfile` from the active
 *     config so callers can pass a rich target to the prompt builders instead of
 *     a bare tier string, enabling provider-aware prompt tailoring (cloud context
 *     window sizing, CLI task-brief depth).
 */

import { getModelTier } from '@ajh/prompts/context-manager';
import type { PromptTier, ProviderKind, ProviderProfile } from '@ajh/prompts/provider';
import type { ActiveAiConfig } from '@ajh/shared';
import type { AiGenerateRequest } from '@ajh/shared/schemas';

import { keys, queryClient } from '@/services/query-client';
import type { AiProvider, PerProviderSettings } from '@/store/preferences-schema';
import { usePreferencesStore } from '@/store/preferences-store';

// ─── Provider kind mapping ────────────────────────────────────────────────────

/** CLI agents: locally-installed headless tools (no API key). */
const CLI_PROVIDERS = new Set<AiProvider>(['claude-code', 'codex', 'gemini-cli', 'antigravity']);

/** Cloud API providers (no local inference). */
const CLOUD_PROVIDERS = new Set<AiProvider>([
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
  'ollama-cloud',
]);

function providerKind(provider: AiProvider): ProviderKind {
  if (CLI_PROVIDERS.has(provider)) return 'cli';
  if (CLOUD_PROVIDERS.has(provider)) return 'cloud';
  return 'ollama';
}

// ─── Resolved tuple ───────────────────────────────────────────────────────────

export interface ActiveProviderContext {
  activeProvider: AiProvider;
  providerSettings: PerProviderSettings | undefined;
  activeModel: string;
}

/**
 * Resolve the active provider + effective model name for imperative prompt
 * shaping (called from async, non-component generation code).
 *
 * Routing (`activeProvider` + `model`) is now BACKEND-owned (task #16), read via
 * React Query's synchronous escape hatch `getQueryData` — never a fresh fetch, so
 * this stays sync. It is boot-prefetched (`AiConfigBoot`) and re-invalidated after
 * onboarding, so the read is warm; a cold read defaults to `ollama` (matching the
 * historical default) rather than blocking.
 *
 * Per-provider TUNING knobs — `modelLimits` (num_ctx sizing) and `effort` — STAY
 * renderer-side (RESOLVED-Q1) and are still read from Zustand here; the backend
 * store owns only routing/egress, never these prompt-shaping knobs.
 *
 * @param fallbackModel - The model name passed by the caller; used when the store
 *   has no model for the active provider yet.
 */
export function resolveActiveProvider(fallbackModel = ''): ActiveProviderContext {
  const routing = queryClient.getQueryData<ActiveAiConfig>(keys.ai.activeConfig);
  const activeProvider = (routing?.activeProvider ?? 'ollama') as AiProvider;
  const activeModel = routing?.model || fallbackModel;
  const providerSettings =
    usePreferencesStore.getState().aiProviderConfig?.providers?.[activeProvider];
  return { activeProvider, providerSettings, activeModel };
}

// ─── Sampling intent (renderer owns intent, adapter owns numbers) ────────────

/**
 * The renderer's declared INTENT for one generation step — never a raw
 * sampling number. Mirrors `AiGenerateRequestSchema.intent`
 * (`packages/shared/src/schemas/index.ts`); each backend provider adapter
 * maps `(model, intent)` to its OWN numbers, or none at all, via
 * `AiProvider::sampling_profile` (`commands/ai_provider/mod.rs`). Sending the
 * SAME hardcoded temperature/penalty numbers to every vendor is what this
 * type exists to stop — see that Rust doc comment for the full rationale.
 */
export type GenerationIntent = NonNullable<AiGenerateRequest['intent']>;

/** One generation step that can carry its own per-model temperature override
 *  (Settings → local model limits → "Custom temperature"). Shared between
 *  `generation.ts` and `resume-ai.ts` so the override key vocabulary can't
 *  drift between the two callers. Each key maps to exactly ONE call site
 *  intent (see `resolveSampling`'s callers in `generation.ts`) — never widen
 *  a key to cover a second, unrelated surface, or a user's single slider
 *  silently retunes generation it never meant to touch. */
export type TemperatureStep =
  'analysis' | 'resume' | 'cover' | 'answers' | 'questions' | 'referral';

/**
 * The user-set per-model, per-step temperature OVERRIDE for one generation
 * step (`LocalModelLimits.tsx`) — the only sampling value in this app a human
 * actually chose. Ollama-only: cloud/CLI providers have no such UI control,
 * so this always resolves to `undefined` for them, leaving the backend
 * adapter's own per-(model, intent) `sampling_profile` to apply instead. The
 * renderer no longer ships a raw default number for any step — see
 * `GenerationIntent`.
 */
export function resolveTemperatureOverride(step: TemperatureStep): number | undefined {
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider();
  if (activeProvider !== 'ollama') return undefined;
  return activeModel
    ? providerSettings?.modelLimits?.[activeModel]?.temperature?.[step]
    : undefined;
}

// ─── Effective prompt tier ────────────────────────────────────────────────────

export type ModelTier = PromptTier;

/**
 * Resolve the effective prompt tier for a given model + provider.
 *
 * This is the single source of truth that replaces the two diverging private
 * `effectiveTier` copies. The previous `resume-ai.ts` copy omitted the
 * `provider` parameter and therefore always returned `'large'` for cloud
 * providers even in compact mode — this version corrects that.
 *
 * Priority:
 *  1. Cloud/CLI providers → always `'large'` (full prompt; they handle it).
 *  2. User `promptQuality` override (`'full'` / `'compact'`).
 *  3. Model-name heuristic via `getModelTier`.
 */
export function resolveEffectiveTier(model: string, provider: AiProvider): ModelTier {
  const { promptQuality } = usePreferencesStore.getState();
  // Cloud and CLI providers always receive the full prompt.
  if (provider !== 'ollama') return 'large';
  if (promptQuality === 'full') return 'large';
  if (promptQuality === 'compact') return 'small';
  return getModelTier(model);
}

// ─── Provider profile (for prompt-tailoring) ─────────────────────────────────

/**
 * Build a {@link ProviderProfile} from the currently active provider config.
 *
 * Passing this to prompt builders instead of a bare tier string enables
 * provider-aware tailoring: cloud context-window sizing and CLI task-brief
 * depth. Native structured output is NOT part of this profile — the Rust
 * provider layer decides it per request (`AiProvider::complete_structured`),
 * where the live provider id, model, and schema are actually known.
 *
 * @param fallbackModel - Forwarded to `resolveActiveProvider`.
 */
export function buildProviderProfile(fallbackModel = ''): ProviderProfile {
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider(fallbackModel);
  const kind = providerKind(activeProvider);

  // For Ollama we derive the sub-tier from the model name so the profile gets
  // the right truncation strategy and depth without a separate `getModelTier` call.
  const tier = resolveEffectiveTier(activeModel, activeProvider);

  // Expose the per-model Ollama context window as `contextWindow` only when it
  // is explicitly configured — cloud providers manage this themselves.
  const localLimits = kind === 'ollama' ? providerSettings?.modelLimits?.[activeModel] : undefined;

  return {
    kind,
    model: activeModel || undefined,
    contextWindow: localLimits?.contextWindow,
    sizeHint: kind === 'ollama' ? tier : undefined,
  };
}
