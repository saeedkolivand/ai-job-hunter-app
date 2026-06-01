import { beforeEach, describe, expect, it } from 'vitest';

import { usePreferencesStore } from './preferences-store';

beforeEach(() => {
  localStorage.clear();
  usePreferencesStore.getState().resetPreferences();
});

describe('usePreferencesStore', () => {
  it('updates scalar preferences', () => {
    const s = usePreferencesStore.getState();
    s.setUserName('Ada');
    s.setLanguage('de');
    s.setOutputTone('formal');
    s.setPerformanceMode('performance');
    s.setPromptQuality('compact');
    s.setDebugMode(true);

    const next = usePreferencesStore.getState();
    expect(next.userName).toBe('Ada');
    expect(next.language).toBe('de');
    expect(next.outputTone).toBe('formal');
    expect(next.performanceMode).toBe('performance');
    expect(next.promptQuality).toBe('compact');
    expect(next.debugMode).toBe(true);
    expect(next.lastUpdated).toBeTruthy();
  });

  it('sets the active provider while preserving existing provider settings', () => {
    const s = usePreferencesStore.getState();
    s.setProviderSettings('openai', { model: 'gpt-4o' });
    s.setActiveProvider('openai');

    const cfg = usePreferencesStore.getState().aiProviderConfig;
    expect(cfg?.activeProvider).toBe('openai');
    expect(cfg?.providers?.openai?.model).toBe('gpt-4o');
  });

  it('merges per-provider settings', () => {
    const s = usePreferencesStore.getState();
    s.setProviderSettings('openai-compatible', { model: 'local', baseUrl: 'http://x' });
    s.setProviderSettings('openai-compatible', { model: 'local-v2' });

    const cfg = usePreferencesStore.getState().aiProviderConfig;
    expect(cfg?.providers?.['openai-compatible']).toEqual({
      model: 'local-v2',
      baseUrl: 'http://x',
    });
  });

  it('stores per-model local limits under ollama, keyed by model and deep-merged', () => {
    const s = usePreferencesStore.getState();
    s.setLocalModelLimits('llama3', { contextWindow: 16384 });
    s.setLocalModelLimits('llama3', { maxTokens: 4096 }); // independent field, merged
    s.setLocalModelLimits('qwen3', { contextWindow: 32768 }); // a different model

    const limits = usePreferencesStore.getState().aiProviderConfig?.providers?.ollama?.modelLimits;
    expect(limits?.llama3).toEqual({ contextWindow: 16384, maxTokens: 4096 });
    expect(limits?.qwen3).toEqual({ contextWindow: 32768 });
  });

  it('marks onboarding complete and resets to defaults', () => {
    usePreferencesStore.getState().setOnboardingComplete();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(true);

    usePreferencesStore.getState().resetPreferences();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(false);
    expect(usePreferencesStore.getState().language).toBe('en');
  });
});
