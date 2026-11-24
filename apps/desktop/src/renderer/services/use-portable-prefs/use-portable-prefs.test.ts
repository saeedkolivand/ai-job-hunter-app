import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { DEFAULT_THEME_PREFS } from '@ajh/ui';

import { defaultPreferences, usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { hydrationOf, localForMigration, portableOf } from './portable-prefs';
import { usePortablePrefsSync } from './use-portable-prefs';

beforeEach(() => {
  usePreferencesStore.setState({ ...defaultPreferences, aiProviderConfig: undefined });
  localStorage.removeItem('ajh-theme');
});
afterEach(() => vi.restoreAllMocks());

const client = (snapshot: { values: Record<string, unknown>; migrated: boolean }) => {
  const fns = {
    get: vi.fn().mockResolvedValue(snapshot),
    migrate: vi.fn().mockResolvedValue({ values: {}, migrated: true }),
    set: vi.fn().mockResolvedValue({ success: true }),
  };
  const mock = createMockClient({
    'portablePrefs.get': fns.get,
    'portablePrefs.migrate': fns.migrate,
    'portablePrefs.set': fns.set,
  });
  return { mock, ...fns };
};

describe('portable preferences migration', () => {
  it('copies only what the user changed when the backend has never been written', async () => {
    usePreferencesStore.setState({
      language: 'de',
      userName: 'Ada',
      performanceMode: 'performance',
    });
    const { mock, ...fn } = client({ values: {}, migrated: false });
    renderHookWithClient(() => usePortablePrefsSync(), { client: mock });

    await waitFor(() => expect(fn.migrate).toHaveBeenCalledTimes(1));
    expect(fn.migrate).toHaveBeenCalledWith({ language: 'de', userName: 'Ada' });
  });

  it('leaves values the backend already holds untouched and applies them locally', async () => {
    usePreferencesStore.setState({ language: 'en' });
    const { mock, ...fn } = client({
      values: { language: 'de', outputTone: 'casual' },
      migrated: true,
    });
    renderHookWithClient(() => usePortablePrefsSync(), { client: mock });

    await waitFor(() => expect(usePreferencesStore.getState().language).toBe('de'));
    expect(usePreferencesStore.getState().outputTone).toBe('casual');
    expect(fn.migrate).not.toHaveBeenCalled();
    // Applying received values is not a user edit: nothing is written back.
    expect(fn.set).not.toHaveBeenCalled();
  });

  it('writes a later user edit through, but never a per-device setting', async () => {
    const { mock, ...fn } = client({ values: {}, migrated: true });
    const { queryClient } = renderHookWithClient(() => usePortablePrefsSync(), { client: mock });
    await waitFor(() =>
      expect(queryClient.getQueryState(keys.portablePrefs.all)?.status).toBe('success')
    );

    await act(async () => {
      usePreferencesStore.setState({ outputTone: 'casual', closeToTray: false });
    });
    await waitFor(() => expect(fn.set).toHaveBeenCalledTimes(1));
    expect(fn.set).toHaveBeenCalledWith({ outputTone: 'casual' });
  });
});

describe('portable value mapping', () => {
  it('never lists per-device or secret settings', () => {
    const everything = {
      ...defaultPreferences,
      performanceMode: 'performance',
      closeToTray: false,
      fetchCompanyLogos: true,
      autoIndexOnUpload: true,
      debugMode: true,
      aiProviderConfig: {
        activeProvider: 'ollama',
        providers: {
          ollama: { model: 'm', effort: 'high', baseUrl: 'http://x', modelLimits: { m: {} } },
        },
      },
    } as const;
    const portable = portableOf(everything);
    for (const banned of [
      'performanceMode',
      'closeToTray',
      'fetchCompanyLogos',
      'autoIndexOnUpload',
    ]) {
      expect(portable).not.toHaveProperty(banned);
    }
    expect(portable).not.toHaveProperty('aiProviderConfig');
    expect(portable.providerEffort).toEqual({ ollama: 'high' });
    expect(JSON.stringify(portable)).not.toContain('http://x');
  });

  it('sends nothing for an untouched install', () => {
    expect(localForMigration(defaultPreferences, DEFAULT_THEME_PREFS)).toEqual({});
  });

  it('ignores malformed received values and a theme with a non-hex accent', () => {
    const out = hydrationOf(
      {
        language: 42,
        outputTone: 'not-a-tone',
        theme: { ...DEFAULT_THEME_PREFS, accentSource: 'custom', accentColor: 'red; x: y' },
      },
      defaultPreferences,
      DEFAULT_THEME_PREFS
    );
    expect(out).toEqual({ patch: null, theme: null });
  });

  it('applies a received theme and reasoning effort', () => {
    const out = hydrationOf(
      { theme: { ...DEFAULT_THEME_PREFS, scheme: 'dark' }, providerEffort: { codex: 'high' } },
      defaultPreferences,
      DEFAULT_THEME_PREFS
    );
    expect(out.theme?.scheme).toBe('dark');
    expect(out.patch?.aiProviderConfig?.providers.codex?.effort).toBe('high');
  });
});
