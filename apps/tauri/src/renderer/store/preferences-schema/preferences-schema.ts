import { z } from 'zod';

import type { PerformanceBackendConfig } from '@ajh/shared';

// Performance modes. `custom` resolves to the user-edited `customPerformance`
// profile; the other three resolve to the fixed presets below.
export const PerformanceModeSchema = z.enum(['low-memory', 'balanced', 'performance', 'custom']);

// Backdrop-blur tier and generic backend tier used by the resolved profile.
/**
 * Backdrop-blur tier. The `'off'` tier is reachable ONLY via a custom profile —
 * the three presets use `'full'` (balanced/performance) or `'reduced'` (low-memory).
 */
export const BlurTierSchema = z.enum(['full', 'reduced', 'off']);
export const PerfTierSchema = z.enum(['low', 'balanced', 'high']);

// Resolved performance profile — the single shape every consumer reads. Presets
// resolve to this; `custom` mode stores one of these directly under
// `customPerformance`.
export const PerformanceProfileSchema = z.object({
  visual: z.object({
    aurora: z.boolean(),
    nebula: z.boolean(),
    // Preset-only: the `performance` preset's second nebula. NOT surfaced in the
    // custom UI; custom profiles keep it false.
    richNebula: z.boolean().default(false),
    cursorGlow: z.boolean(),
    blur: BlurTierSchema,
    animations: z.boolean(),
  }),
  backend: z.object({
    concurrency: PerfTierSchema,
    keepAlive: PerfTierSchema,
    cache: PerfTierSchema,
  }),
});

export type PerformanceProfile = z.infer<typeof PerformanceProfileSchema>;
export type BlurTier = z.infer<typeof BlurTierSchema>;
export type PerfTier = z.infer<typeof PerfTierSchema>;

// Prompt quality — controls which prompt variant is sent to the model
// auto    = detect from model tier (default, safe for all providers)
// full    = always use full multi-perspective prompts regardless of model
// compact = always use compact prompts (small-model optimised, fastest)
export const PromptQualitySchema = z.enum(['auto', 'full', 'compact']);

// Output tone options
export const OutputToneSchema = z.enum(['professional', 'casual', 'formal', 'creative']);

// AI model preference
export const AIModelPreferenceSchema = z.object({
  defaultModel: z.string().optional(),
  temperature: z.number().min(0).max(2).default(0.7),
  maxTokens: z.number().min(1).max(8192).default(2048),
});

// AI provider selection (API key stored in OS keychain, not here)
export const AiProviderSchema = z.enum([
  'ollama',
  // Ollama Cloud — hosted Ollama models via its OpenAI-compatible endpoint; the
  // same account key also powers Ollama Web Search for company research.
  'ollama-cloud',
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
  // CLI agents — locally-installed headless tools (no API key; own login).
  'claude-code',
  'codex',
  'gemini-cli',
]);

// Per-local-model generation limits (Ollama only). Keyed by model name so each
// model remembers its own context window + max output. Cloud/CLI providers ignore these.
export const LocalModelLimitsSchema = z.object({
  contextWindow: z.number().int().min(512).max(131072).optional(),
  maxTokens: z.number().int().min(1).max(32768).optional(),
  // Optional per-model, per-step sampling temperatures (Ollama only). undefined =
  // use the app's per-step defaults. Each step is independently settable; a step
  // left undefined falls back to its default. (Migrates the legacy single-number
  // shape by dropping it.)
  temperature: z
    .preprocess(
      (v) => (typeof v === 'number' ? undefined : v),
      z
        .object({
          analysis: z.number().min(0).max(2).optional(),
          resume: z.number().min(0).max(2).optional(),
          cover: z.number().min(0).max(2).optional(),
          answers: z.number().min(0).max(2).optional(),
          referral: z.number().min(0).max(2).optional(),
        })
        .optional()
    )
    .optional(),
});

// Per-provider settings (model choice, optional base URL, optional CLI effort)
export const PerProviderSettingsSchema = z.object({
  model: z.string().default(''),
  baseUrl: z.string().optional(),
  // Reasoning effort for CLI agents that support it (e.g. Codex: low/medium/high).
  effort: z.string().optional(),
  // Per-model generation limits, keyed by model name (local/Ollama only).
  // Optional so existing per-provider settings (and the many `{ model }` literals)
  // stay valid; readers default it to `{}`.
  modelLimits: z.record(z.string(), LocalModelLimitsSchema).optional(),
});

// Multi-provider config: each provider stores its own settings independently;
// activeProvider is the one used for generation.
export const AiProviderConfigSchema = z.object({
  activeProvider: AiProviderSchema.default('ollama'),
  providers: z.record(z.string(), PerProviderSettingsSchema).default({}),
});

export type AiProvider = z.infer<typeof AiProviderSchema>;
export type AiProviderConfig = z.infer<typeof AiProviderConfigSchema>;
export type PerProviderSettings = z.infer<typeof PerProviderSettingsSchema>;
export type LocalModelLimits = z.infer<typeof LocalModelLimitsSchema>;

// Resume preference
export const ResumePreferenceSchema = z.object({
  defaultId: z.string().optional(),
  autoIndex: z.boolean().default(true),
  autoParse: z.boolean().default(true),
});

// Applicant preferences — user-supplied facts a résumé can't answer (salary,
// availability, notice, remote). Fed to the cover letter (market inclusions such
// as the DACH salary expectation + earliest start date) and to autopilot
// application answers. User-supplied ONLY; never inferred or auto-filled.
export const ApplicantPreferencesSchema = z.object({
  salaryExpectation: z.string().optional(),
  earliestStartDate: z.string().optional(),
  noticePeriod: z.string().optional(),
  remotePreference: z.string().optional(),
});

// Main preferences schema
export const PreferencesSchema = z.object({
  version: z.number().default(1),

  // User Profile
  userName: z.string().optional(),

  // AI Preferences
  language: z.string().default('en'),
  aiModel: AIModelPreferenceSchema.optional(),
  aiProviderConfig: AiProviderConfigSchema.optional(),
  outputTone: OutputToneSchema.default('professional'),

  // Resume Preferences
  resume: ResumePreferenceSchema.optional(),

  // Applicant preferences (salary, start date, notice, remote) — user-supplied.
  applicant: ApplicantPreferencesSchema.optional(),

  // Performance Preferences
  performanceMode: PerformanceModeSchema.default('balanced'),

  // User-edited profile used when performanceMode === 'custom'. Seeded from the
  // balanced preset the first time the user picks the Custom card.
  customPerformance: PerformanceProfileSchema.optional(),

  // Prompt quality — which prompt variant to send to the AI model
  promptQuality: PromptQualitySchema.default('auto'),

  // Developer / debug
  debugMode: z.boolean().default(false),

  // Scoring
  semanticScoring: z.boolean().default(false),

  // Window close behaviour — keep the app running in the tray/menu-bar when the
  // window is closed (default on). Pushed to the Rust shell on boot + on change;
  // the shell's window-close handler reads the live flag.
  closeToTray: z.boolean().default(true),

  // Onboarding
  onboardingCompleted: z.boolean().default(false),

  // One-time nudge to review/complete the contact profile before the first
  // résumé / cover-letter generation.
  contactPromptSeen: z.boolean().default(false),

  // Metadata
  lastUpdated: z.string().optional(),
});

export type Preferences = z.infer<typeof PreferencesSchema>;
export type PerformanceMode = z.infer<typeof PerformanceModeSchema>;

// ── Performance presets & resolvers ─────────────────────────────────────────

/**
 * Fixed profiles for the three preset modes. These reproduce the historical
 * behavior exactly (low-memory = renders nothing + reduced blur + no animations;
 * balanced = aurora + 1 nebula + cursor glow; performance = + 2nd nebula).
 */
export const PERFORMANCE_PRESETS: Record<
  'low-memory' | 'balanced' | 'performance',
  PerformanceProfile
> = {
  'low-memory': {
    visual: {
      aurora: false,
      nebula: false,
      richNebula: false,
      cursorGlow: false,
      blur: 'reduced',
      animations: false,
    },
    backend: { concurrency: 'low', keepAlive: 'low', cache: 'low' },
  },
  balanced: {
    visual: {
      aurora: true,
      nebula: true,
      richNebula: false,
      cursorGlow: true,
      blur: 'full',
      animations: true,
    },
    backend: { concurrency: 'balanced', keepAlive: 'balanced', cache: 'balanced' },
  },
  performance: {
    visual: {
      aurora: true,
      nebula: true,
      richNebula: true,
      cursorGlow: true,
      blur: 'full',
      animations: true,
    },
    backend: { concurrency: 'high', keepAlive: 'high', cache: 'high' },
  },
};

/**
 * Resolve the active mode + optional custom profile into a single profile.
 * `custom` falls back to the balanced preset until the user has seeded a custom
 * profile (defensive — the UI seeds it on first selection).
 */
export function resolveProfile(
  prefs: Pick<Preferences, 'performanceMode' | 'customPerformance'>
): PerformanceProfile {
  if (prefs.performanceMode === 'custom') {
    return prefs.customPerformance ?? PERFORMANCE_PRESETS.balanced;
  }
  return PERFORMANCE_PRESETS[prefs.performanceMode];
}

// Tier→number tables for the backend IPC payload. cache 'low' = minimal,
// 'balanced' = balanced, 'high' = generous (no expiry / unbounded rows).
const CONCURRENCY_BY_TIER: Record<PerfTier, number> = { low: 1, balanced: 2, high: 4 };
const KEEP_ALIVE_SECS_BY_TIER: Record<PerfTier, number> = { low: 0, balanced: 300, high: 1800 };
const CACHE_TTL_SECS_BY_TIER: Record<PerfTier, number | null> = {
  low: 86400,
  balanced: 604800,
  high: null,
};
const CACHE_MAX_ROWS_BY_TIER: Record<PerfTier, number | null> = {
  low: 250,
  balanced: 2000,
  high: null,
};

/**
 * Resolve a profile's backend tiers into the concrete `PerformanceBackendConfig`
 * pushed over IPC to the Rust shell. Pure — no side effects.
 */
export function resolveBackendConfig(
  mode: PerformanceMode,
  profile: PerformanceProfile
): PerformanceBackendConfig {
  return {
    mode,
    concurrency: CONCURRENCY_BY_TIER[profile.backend.concurrency],
    keepAliveSecs: KEEP_ALIVE_SECS_BY_TIER[profile.backend.keepAlive],
    cacheTtlSecs: CACHE_TTL_SECS_BY_TIER[profile.backend.cache],
    cacheMaxRows: CACHE_MAX_ROWS_BY_TIER[profile.backend.cache],
  };
}
export type PromptQuality = z.infer<typeof PromptQualitySchema>;
export type OutputTone = z.infer<typeof OutputToneSchema>;
export type AIModelPreference = z.infer<typeof AIModelPreferenceSchema>;
export type ResumePreference = z.infer<typeof ResumePreferenceSchema>;
export type ApplicantPreferences = z.infer<typeof ApplicantPreferencesSchema>;
