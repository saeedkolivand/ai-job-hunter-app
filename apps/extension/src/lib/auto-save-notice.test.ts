/**
 * The one-shot save-answers-on-submit auto-save notice (`lib/auto-save-notice.ts`).
 */

import { describe, expect, it, vi } from 'vitest';

// An in-memory `storage.session` area — mirrors answer-state.test.ts's own mock.
vi.mock('@wxt-dev/browser', () => {
  const store: Record<string, unknown> = {};
  return {
    browser: {
      storage: {
        session: {
          get: vi.fn((key: string) => Promise.resolve({ [key]: store[key] })),
          set: vi.fn((entries: Record<string, unknown>) => {
            Object.assign(store, entries);
            return Promise.resolve();
          }),
          remove: vi.fn((key: string) => {
            delete store[key];
            return Promise.resolve();
          }),
        },
      },
    },
  };
});

import { setAutoSaveNotice, takeAutoSaveNotice } from './auto-save-notice';

describe('setAutoSaveNotice / takeAutoSaveNotice', () => {
  it('returns null when nothing was ever set', async () => {
    expect(await takeAutoSaveNotice()).toBeNull();
  });

  it('round-trips a set notice', async () => {
    await setAutoSaveNotice('Saved 1 answer from this submit.');
    expect(await takeAutoSaveNotice()).toBe('Saved 1 answer from this submit.');
  });

  it('is READ-ONCE — a second take returns null', async () => {
    await setAutoSaveNotice('Saved 2 answers from this submit.');
    await takeAutoSaveNotice();
    expect(await takeAutoSaveNotice()).toBeNull();
  });
});
