/**
 * preferences-store — custom performance mode tests.
 *
 * Covers:
 *  - setCustomPerformance writes the profile and bumps lastUpdated.
 *  - useResolvedPerformanceProfile (via the selector) returns the preset when
 *    mode is a preset and the custom profile when mode='custom'.
 *  - setPerformanceMode('custom') switches mode; combined with setCustomPerformance
 *    the resolver returns the custom profile.
 */
import { beforeEach, describe, expect, it } from 'vitest';

import {
  PERFORMANCE_PRESETS,
  type PerformanceProfile,
  resolveProfile,
} from '../preferences-schema';
import { usePreferencesStore } from './preferences-store';

beforeEach(() => {
  localStorage.clear();
  usePreferencesStore.getState().resetPreferences();
  // resetPreferences() merges defaultPreferences but does not explicitly clear
  // fields absent from defaultPreferences (like customPerformance). Clear it
  // explicitly so test isolation is guaranteed.
  usePreferencesStore.setState({ customPerformance: undefined });
});

const CUSTOM_PROFILE: PerformanceProfile = {
  visual: {
    aurora: true,
    nebula: false,
    richNebula: false,
    cursorGlow: false,
    blur: 'off',
    animations: false,
  },
  backend: { concurrency: 'high', keepAlive: 'low', cache: 'balanced' },
};

describe('setCustomPerformance', () => {
  it('writes the custom profile into the store', () => {
    usePreferencesStore.getState().setCustomPerformance(CUSTOM_PROFILE);
    const stored = usePreferencesStore.getState().customPerformance;
    expect(stored).toEqual(CUSTOM_PROFILE);
  });

  it('bumps lastUpdated after writing the profile', () => {
    const before = usePreferencesStore.getState().lastUpdated;
    // `before` is always defined after resetPreferences() — guard explicitly so
    // we never silently skip the timestamp comparison (no `if (before)` wrapper).
    if (typeof before !== 'string')
      throw new Error('lastUpdated must be a string before setCustomPerformance');

    usePreferencesStore.getState().setCustomPerformance(CUSTOM_PROFILE);

    const after = usePreferencesStore.getState().lastUpdated;
    // `after` must be a non-empty string (never null / undefined / number).
    if (typeof after !== 'string')
      throw new Error('lastUpdated must remain a string after setCustomPerformance');

    // The timestamp must be the same or newer — never rolled back.
    expect(new Date(after).getTime()).toBeGreaterThanOrEqual(new Date(before).getTime());
  });

  it('can be updated multiple times (last write wins)', () => {
    usePreferencesStore.getState().setCustomPerformance(CUSTOM_PROFILE);
    const updated: PerformanceProfile = {
      ...CUSTOM_PROFILE,
      visual: { ...CUSTOM_PROFILE.visual, aurora: false },
    };
    usePreferencesStore.getState().setCustomPerformance(updated);
    const stored = usePreferencesStore.getState().customPerformance;
    expect(stored?.visual.aurora).toBe(false);
  });
});

describe('useResolvedPerformanceProfile selector', () => {
  it('returns the balanced preset when mode=balanced and no custom profile', () => {
    const state = usePreferencesStore.getState();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    expect(resolved).toEqual(PERFORMANCE_PRESETS.balanced);
  });

  it('returns the low-memory preset when mode=low-memory', () => {
    usePreferencesStore.getState().setPerformanceMode('low-memory');
    const state = usePreferencesStore.getState();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    expect(resolved).toEqual(PERFORMANCE_PRESETS['low-memory']);
  });

  it('returns the performance preset when mode=performance', () => {
    usePreferencesStore.getState().setPerformanceMode('performance');
    const state = usePreferencesStore.getState();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    expect(resolved).toEqual(PERFORMANCE_PRESETS.performance);
  });

  it('returns the custom profile when mode=custom and a custom profile is set', () => {
    usePreferencesStore.getState().setPerformanceMode('custom');
    usePreferencesStore.getState().setCustomPerformance(CUSTOM_PROFILE);
    const state = usePreferencesStore.getState();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    expect(resolved).toEqual(CUSTOM_PROFILE);
  });

  it('falls back to balanced preset when mode=custom but no custom profile is stored', () => {
    usePreferencesStore.getState().setPerformanceMode('custom');
    // After reset, customPerformance is undefined.
    const state = usePreferencesStore.getState();
    expect(state.customPerformance).toBeUndefined();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    expect(resolved).toEqual(PERFORMANCE_PRESETS.balanced);
  });

  it('updates the resolved profile when mode switches from custom back to balanced', () => {
    usePreferencesStore.getState().setPerformanceMode('custom');
    usePreferencesStore.getState().setCustomPerformance(CUSTOM_PROFILE);
    usePreferencesStore.getState().setPerformanceMode('balanced');
    const state = usePreferencesStore.getState();
    const resolved = resolveProfile({
      performanceMode: state.performanceMode,
      customPerformance: state.customPerformance,
    });
    // The custom profile is still stored but not returned (mode is now 'balanced').
    expect(resolved).toEqual(PERFORMANCE_PRESETS.balanced);
    expect(state.customPerformance).toEqual(CUSTOM_PROFILE); // preserved in store
  });
});
