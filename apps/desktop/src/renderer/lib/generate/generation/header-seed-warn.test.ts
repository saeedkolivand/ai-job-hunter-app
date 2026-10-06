import { describe, expect, it, vi } from 'vitest';

import { seedHeaderFromContactProfile } from './header-seed';
import { installGenerationHooks, registerWithContactProfile } from './test-support';

installGenerationHooks();

// JSON.stringify drops an Error's message, so render it explicitly.
const logged = (calls: unknown[]) =>
  JSON.stringify(calls, (_k, v) => (v instanceof Error ? `${v.name}: ${v.message}` : v));

const SENTINEL = 'PII-SENTINEL jane@example.com';
const META = { targetLanguage: 'en' } as Parameters<typeof seedHeaderFromContactProfile>[1];

describe('seedHeaderFromContactProfile — failure logs carry no raw error message', () => {
  it.each(['get', 'headerLine'] as const)('contactProfile.%s rejection', async (which) => {
    registerWithContactProfile({
      get: vi
        .fn()
        .mockImplementation(() =>
          which === 'get' ? Promise.reject(new Error(SENTINEL)) : Promise.resolve({})
        ),
      headerLine: vi
        .fn()
        .mockImplementation(() =>
          which === 'headerLine' ? Promise.reject(new Error(SENTINEL)) : Promise.resolve('x')
        ),
    });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const out = await seedHeaderFromContactProfile('Name\nbody', META, 'en');
    expect(out).toBe('Name\nbody');
    expect(warn).toHaveBeenCalled();
    expect(logged(warn.mock.calls)).not.toContain('PII-SENTINEL');
  });
});
