import { z } from 'zod';

// Remote preference options
export const RemotePreferenceSchema = z.enum(['remote', 'hybrid', 'on-site', 'any']);

// Seniority levels
export const SenioritySchema = z.enum([
  'entry',
  'junior',
  'mid',
  'senior',
  'lead',
  'principal',
  'any',
]);

// Performance modes
export const PerformanceModeSchema = z.enum(['low-memory', 'balanced', 'performance']);

// Output tone options
export const OutputToneSchema = z.enum(['professional', 'casual', 'formal', 'creative']);

// Location preference
export const LocationPreferenceSchema = z.object({
  city: z.string().optional(),
  country: z.string().optional(),
  region: z.string().optional(),
  radius: z.number().min(0).max(500).optional(), // km
});

// Tech stack item
export const TechStackItemSchema = z.object({
  name: z.string(),
  category: z.enum(['language', 'framework', 'database', 'tool', 'other']),
  proficiency: z.enum(['beginner', 'intermediate', 'advanced', 'expert']).optional(),
});

// Salary expectations
export const SalaryExpectationSchema = z.object({
  min: z.number().min(0).optional(),
  max: z.number().min(0).optional(),
  currency: z.string().default('USD'),
  period: z.enum(['hourly', 'monthly', 'yearly']).default('yearly'),
});

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

  // Job Preferences
  location: LocationPreferenceSchema.optional(),
  remote: RemotePreferenceSchema.default('any'),
  techStack: z.array(TechStackItemSchema).default([]),
  seniority: SenioritySchema.default('any'),
  salary: SalaryExpectationSchema.optional(),

  // Resume Preferences
  resume: ResumePreferenceSchema.optional(),

  // Performance Preferences
  performanceMode: PerformanceModeSchema.default('balanced'),

  // Onboarding
  onboardingCompleted: z.boolean().default(false),

  // Metadata
  lastUpdated: z.string().optional(),
});

export type Preferences = z.infer<typeof PreferencesSchema>;
export type RemotePreference = z.infer<typeof RemotePreferenceSchema>;
export type Seniority = z.infer<typeof SenioritySchema>;
export type PerformanceMode = z.infer<typeof PerformanceModeSchema>;
export type OutputTone = z.infer<typeof OutputToneSchema>;
export type LocationPreference = z.infer<typeof LocationPreferenceSchema>;
export type TechStackItem = z.infer<typeof TechStackItemSchema>;
export type SalaryExpectation = z.infer<typeof SalaryExpectationSchema>;
export type AIModelPreference = z.infer<typeof AIModelPreferenceSchema>;
export type ResumePreference = z.infer<typeof ResumePreferenceSchema>;
