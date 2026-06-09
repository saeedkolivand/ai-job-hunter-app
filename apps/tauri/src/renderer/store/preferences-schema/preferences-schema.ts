import { z } from 'zod';

// Performance modes
export const PerformanceModeSchema = z.enum(['low-memory', 'balanced', 'performance']);

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

  // Prompt quality — which prompt variant to send to the AI model
  promptQuality: PromptQualitySchema.default('auto'),

  // Developer / debug
  debugMode: z.boolean().default(false),

  // Scoring
  semanticScoring: z.boolean().default(false),

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
export type PromptQuality = z.infer<typeof PromptQualitySchema>;
export type OutputTone = z.infer<typeof OutputToneSchema>;
export type AIModelPreference = z.infer<typeof AIModelPreferenceSchema>;
export type ResumePreference = z.infer<typeof ResumePreferenceSchema>;
export type ApplicantPreferences = z.infer<typeof ApplicantPreferencesSchema>;
