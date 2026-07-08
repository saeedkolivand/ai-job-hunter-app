/**
 * Provider-aware prompt layer.
 *
 * Prompts adapt to the three AI provider classes the app supports — local
 * **ollama** models, **cloud** API keys, and **cli** agents — choosing prompt
 * verbosity, truncation strategy, schema variant, and output-format. This module
 * stays PURE: it describes provider capabilities declaratively (flags + metadata);
 * the real API calls and CLI invocations live in the caller.
 */

import {
  detectModelSize,
  LARGE_MODEL_STRATEGY,
  MEDIUM_MODEL_STRATEGY,
  SMALL_MODEL_STRATEGY,
  type TruncationStrategy,
} from '../context-manager/index.js';

export type ProviderKind = 'ollama' | 'cloud' | 'cli';
export type PromptTier = 'large' | 'medium' | 'small';

export interface ProviderProfile {
  kind: ProviderKind;
  /** Raw model / tag name (e.g. `llama3.2:1b`, `gpt-4o`). */
  model?: string;
  /** Context window in tokens, if known — sizes cloud input budgets. */
  contextWindow?: number;
  /** Native JSON-schema / tool-use support (cloud). */
  supportsStructuredOutput?: boolean;
  /** Explicit ollama sub-tier; otherwise derived from `model`. */
  sizeHint?: PromptTier;
}

/**
 * What every builder accepts: either a legacy tier string (still works verbatim →
 * backward compatible) or a full {@link ProviderProfile}.
 */
export type PromptTarget = PromptTier | ProviderProfile;

/** Prompt framing depth. */
export type PromptDepth = 'brief' | 'full' | 'task';

export interface ResolvedProfile {
  kind: ProviderKind;
  tier: PromptTier;
  /** brief = compact/imperative, full = multi-perspective, task = agent task brief. */
  depth: PromptDepth;
  /** Output schema variant for analysis JSON. */
  schema: 'compact' | 'full';
  /** Whether to request the rich `rewrites` field. */
  includeRewrites: boolean;
  /** Whether the caller can request native structured output. */
  structuredOutput: boolean;
  /** Resume truncation strategy. */
  truncation: TruncationStrategy;
  /** Char cap for the resume slice. */
  resumeChars: number;
  /** Char cap for the job-ad slice. */
  jobAdChars: number;
}

function isProfile(t: PromptTarget): t is ProviderProfile {
  return typeof t === 'object' && t !== null && 'kind' in t;
}

function resolveTruncation(profile: ProviderProfile, tier: PromptTier): TruncationStrategy {
  if (profile.kind === 'cloud') {
    // Minimal truncation — size to the real context window, preserve all sections.
    const window = profile.contextWindow ?? 0;
    const maxTokens = window > 0 ? Math.min(Math.floor(window * 0.5), 24000) : 12000;
    return { ...LARGE_MODEL_STRATEGY, maxTokens, summarizeSections: [], dropSections: [] };
  }
  if (profile.kind === 'cli') {
    // Moderate — the agent can re-expand context itself if needed.
    return { ...LARGE_MODEL_STRATEGY, maxTokens: 8000 };
  }
  // ollama: aggressive, by sub-tier.
  if (tier === 'small') return SMALL_MODEL_STRATEGY;
  if (tier === 'medium') return MEDIUM_MODEL_STRATEGY;
  return LARGE_MODEL_STRATEGY;
}

/**
 * Normalize a {@link PromptTarget} into the concrete decisions a builder needs.
 *
 * - **cli** → `task` brief (self-verifying, full schema, moderate truncation).
 * - **cloud** → `full` multi-perspective prompt + rich schema + native structured
 *   output, minimal truncation sized to the context window.
 * - **ollama** → `full` only for a large local model, else `brief` (compact prompt,
 *   compact schema, aggressive truncation). Unknown local models default to the
 *   smaller/safer `brief` path via {@link detectModelSize}.
 *
 * A bare tier string is treated as a local model with that size — which finally
 * splits the old "medium == large" conflation (medium → brief).
 */
export function resolveProfile(target: PromptTarget = 'large'): ResolvedProfile {
  const profile: ProviderProfile = isProfile(target)
    ? target
    : { kind: 'ollama', sizeHint: target };

  const tier: PromptTier =
    profile.sizeHint ?? (profile.model ? detectModelSize(profile.model) : 'medium');

  const depth: PromptDepth =
    profile.kind === 'cli'
      ? 'task'
      : profile.kind === 'cloud'
        ? 'full'
        : tier === 'large'
          ? 'full'
          : 'brief';

  const schema: 'compact' | 'full' = depth === 'brief' ? 'compact' : 'full';
  const structuredOutput = profile.supportsStructuredOutput ?? profile.kind === 'cloud';

  return {
    kind: profile.kind,
    tier,
    depth,
    schema,
    includeRewrites: schema === 'full',
    structuredOutput,
    truncation: resolveTruncation(profile, tier),
    // resumeChars is retained for backward-compat / callers that still slice;
    // the generation builders now feed the résumé via `truncation` +
    // truncateResume() instead, which preserves whole high-value sections.
    resumeChars: depth === 'brief' ? 2500 : profile.kind === 'cloud' ? 8000 : 5000,
    // Job ads are not the candidate's data and often bury requirements late, so
    // give the extractor/generator more of the ad to read.
    jobAdChars: depth === 'brief' ? 2500 : profile.kind === 'cloud' ? 6000 : 5000,
  };
}
