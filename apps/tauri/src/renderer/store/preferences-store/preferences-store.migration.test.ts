import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const KEY = 'ai-job-hunter-preferences';

// The persist middleware runs migratePreferences() during hydration. Seeding an
// old persisted payload before importing the store exercises each migration
// branch (v0→v1 baseline, v1→v2 provider-config flatten, v2→v3 promptQuality).
beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});

afterEach(() => {
  localStorage.clear();
});

async function hydrate() {
  const mod = await import('./preferences-store');
  await mod.usePreferencesStore.persist.rehydrate();
  return mod.usePreferencesStore.getState();
}

describe('preferences-store migrations', () => {
  it('migrates a v0 payload through every step', async () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({
        version: 0,
        state: {
          language: 'de',
          aiProviderConfig: { provider: 'openai', model: 'gpt-4o', baseUrl: 'http://x' },
        },
      })
    );

    const state = await hydrate();
    expect(state.language).toBe('de');
    // v2→v3 added promptQuality
    expect(state.promptQuality).toBe('auto');
    // v1→v2 flattened the legacy provider config
    expect(state.aiProviderConfig?.activeProvider).toBe('openai');
    expect(state.aiProviderConfig?.providers?.openai).toEqual({
      model: 'gpt-4o',
      baseUrl: 'http://x',
    });
  });

  it('adds promptQuality when migrating a v2 payload', async () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({ version: 2, state: { language: 'en', outputTone: 'formal' } })
    );
    const state = await hydrate();
    expect(state.promptQuality).toBe('auto');
    expect(state.outputTone).toBe('formal');
  });
});
