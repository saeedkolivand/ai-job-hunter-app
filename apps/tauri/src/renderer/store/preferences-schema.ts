import { z } from 'zod';

// Performance modes
export const PerformanceModeSchema = z.enum(['low-memory', 'balanced', 'performance']);

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
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
]);

// Per-provider settings (model choice, optional base URL)
export const PerProviderSettingsSchema = z.object({
  model: z.string().default(''),
  baseUrl: z.string().optional(),
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

// Resume preference
export const ResumePreferenceSchema = z.object({
  defaultId: z.string().optional(),
  autoIndex: z.boolean().default(true),
  autoParse: z.boolean().default(true),
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

  // Performance Preferences
  performanceMode: PerformanceModeSchema.default('balanced'),

  // Developer / debug
  debugMode: z.boolean().default(false),

  // Onboarding
  onboardingCompleted: z.boolean().default(false),

  // Metadata
  lastUpdated: z.string().optional(),
});

export type Preferences = z.infer<typeof PreferencesSchema>;
export type PerformanceMode = z.infer<typeof PerformanceModeSchema>;
export type OutputTone = z.infer<typeof OutputToneSchema>;
export type AIModelPreference = z.infer<typeof AIModelPreferenceSchema>;
export type ResumePreference = z.infer<typeof ResumePreferenceSchema>;
