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

  it('adds fetchCompanyLogos=false when migrating a v3 payload (existing users default OFF)', async () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({ version: 3, state: { language: 'en', promptQuality: 'full' } })
    );
    const state = await hydrate();
    expect(state.fetchCompanyLogos).toBe(false);
    // Existing preferences are preserved
    expect(state.promptQuality).toBe('full');
  });

  it('fetchCompanyLogos defaults to false on a fresh store (no persisted data)', async () => {
    // No localStorage seed — fresh hydration
    const state = await hydrate();
    expect(state.fetchCompanyLogos).toBe(false);
  });

  it('adds generationDepth=fast when migrating a v4 payload (never upgrade spend silently)', async () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({ version: 4, state: { language: 'en', fetchCompanyLogos: true } })
    );
    const state = await hydrate();
    // The VERSION MARKER is what proves the step ran. The field alone doesn't:
    // zustand's merge already fills it from `defaultPreferences`, so a test
    // asserting only `generationDepth === 'fast'` passes with the step deleted
    // (verified by mutation) and guards nothing.
    expect(state.version).toBe(5);
    expect(state.generationDepth).toBe('fast');
    expect(state.fetchCompanyLogos).toBe(true);
  });
});
