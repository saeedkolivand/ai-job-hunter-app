import { describe, expect, it } from 'vitest';

import {
  AIModelPreferenceSchema,
  AiProviderConfigSchema,
  OutputToneSchema,
  PerformanceModeSchema,
  PreferencesSchema,
  PromptQualitySchema,
} from './preferences-schema';

describe('PreferencesSchema', () => {
  it('fills sensible defaults from an empty object', () => {
    const prefs = PreferencesSchema.parse({});
    expect(prefs.version).toBe(1);
    expect(prefs.language).toBe('en');
    expect(prefs.outputTone).toBe('professional');
    expect(prefs.performanceMode).toBe('balanced');
    expect(prefs.promptQuality).toBe('auto');
    expect(prefs.debugMode).toBe(false);
    expect(prefs.onboardingCompleted).toBe(false);
  });

  it('rejects invalid enum values', () => {
    expect(() => PerformanceModeSchema.parse('turbo')).toThrow();
    expect(() => OutputToneSchema.parse('snarky')).toThrow();
    expect(() => PromptQualitySchema.parse('medium')).toThrow();
  });
});

describe('AIModelPreferenceSchema', () => {
  it('defaults temperature and maxTokens', () => {
    const parsed = AIModelPreferenceSchema.parse({});
    expect(parsed.temperature).toBeCloseTo(0.7);
    expect(parsed.maxTokens).toBe(2048);
  });

  it('clamps temperature to 0–2 and maxTokens to 1–8192', () => {
    expect(() => AIModelPreferenceSchema.parse({ temperature: 2.5 })).toThrow();
    expect(() => AIModelPreferenceSchema.parse({ maxTokens: 9000 })).toThrow();
  });
});

describe('AiProviderConfigSchema', () => {
  it('defaults to ollama with no providers', () => {
    const parsed = AiProviderConfigSchema.parse({});
    expect(parsed.activeProvider).toBe('ollama');
    expect(parsed.providers).toEqual({});
  });

  it('rejects an unknown active provider', () => {
    expect(() => AiProviderConfigSchema.parse({ activeProvider: 'cohere' })).toThrow();
  });
});
