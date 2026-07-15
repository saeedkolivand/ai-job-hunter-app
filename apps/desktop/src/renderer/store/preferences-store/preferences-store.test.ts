import { beforeEach, describe, expect, it, vi } from 'vitest';

// ── onboarding-mirror — mock so store tests are pure Zustand (no Tauri store) ──
// vi.hoisted() ensures the spies are initialized before vi.mock() hoisting runs.

const { mockClearOnboardingMirror, mockMarkOnboardingComplete } = vi.hoisted(() => ({
  mockClearOnboardingMirror: vi.fn().mockResolvedValue(undefined),
  mockMarkOnboardingComplete: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/lib/onboarding-mirror', () => ({
  clearOnboardingMirror: mockClearOnboardingMirror,
  markOnboardingComplete: mockMarkOnboardingComplete,
}));

import { usePreferencesStore } from './preferences-store';

beforeEach(() => {
  localStorage.clear();
  usePreferencesStore.getState().resetPreferences();
  mockClearOnboardingMirror.mockClear();
  mockMarkOnboardingComplete.mockClear();
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

  // Routing (activeProvider) moved to the backend store (task #16); the surviving
  // renderer-side write is the per-provider `effort` CLI tuning knob.
  it('stores a per-provider effort (the surviving renderer-side tuning knob)', () => {
    const s = usePreferencesStore.getState();
    s.setProviderSettings('codex', { effort: 'high' });

    const cfg = usePreferencesStore.getState().aiProviderConfig;
    expect(cfg?.providers?.codex?.effort).toBe('high');
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

  it('re-arms onboarding via resetOnboarding (without touching other prefs)', () => {
    const s = usePreferencesStore.getState();
    s.setOnboardingComplete();
    s.setLanguage('de');
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(true);

    usePreferencesStore.getState().resetOnboarding();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(false);
    // Unrelated prefs survive (unlike resetPreferences).
    expect(usePreferencesStore.getState().language).toBe('de');
  });

  it('resetOnboarding calls clearOnboardingMirror() and sets onboardingCompleted to false', () => {
    const s = usePreferencesStore.getState();
    s.setOnboardingComplete();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(true);

    usePreferencesStore.getState().resetOnboarding();

    expect(mockClearOnboardingMirror).toHaveBeenCalledOnce();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(false);
  });

  it('setOnboardingComplete calls markOnboardingComplete() and sets onboardingCompleted to true', () => {
    usePreferencesStore.getState().setOnboardingComplete();

    expect(mockMarkOnboardingComplete).toHaveBeenCalledOnce();
    expect(usePreferencesStore.getState().onboardingCompleted).toBe(true);
  });

  it('records recent locations most-recent-first, de-duplicated and capped at 5', () => {
    const s = usePreferencesStore.getState();
    expect(usePreferencesStore.getState().recentLocations).toEqual([]);

    s.addRecentLocation('Berlin');
    s.addRecentLocation('  '); // blank ignored
    s.addRecentLocation('London');
    s.addRecentLocation('Berlin'); // dedup → moves to front
    expect(usePreferencesStore.getState().recentLocations).toEqual(['Berlin', 'London']);

    ['Paris', 'Madrid', 'Rome', 'Vienna'].forEach((l) => s.addRecentLocation(l));
    const recent = usePreferencesStore.getState().recentLocations;
    expect(recent).toHaveLength(5);
    expect(recent[0]).toBe('Vienna'); // newest first
    expect(recent).not.toContain('London'); // oldest dropped past the cap
  });
});
