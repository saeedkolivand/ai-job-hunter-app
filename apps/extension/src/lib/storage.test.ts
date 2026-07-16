/**
 * Unit tests for apps/extension/src/lib/storage.ts.
 *
 * We intercept @wxt-dev/browser with vi.mock before the module graph resolves,
 * providing a minimal browser.storage.local stub backed by an in-memory object.
 *
 * vi.hoisted() is used to create the in-memory store and mock fns BEFORE the
 * vi.mock factory runs (vi.mock is hoisted above imports by vitest's transform).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import {
  clearToken,
  getAnswerToolsExpanded,
  getToken,
  looksLikeToken,
  setAnswerToolsExpanded,
  setToken,
} from './storage';

// ── Hoisted store (must be created before vi.mock factory executes) ───────────

const { _store, storageLocal } = vi.hoisted(() => {
  const _store: Record<string, unknown> = {};
  const storageLocal = {
    get: vi.fn(async (key: string) => ({ [key]: _store[key] })),
    set: vi.fn(async (items: Record<string, unknown>) => {
      Object.assign(_store, items);
    }),
    remove: vi.fn(async (key: string) => {
      delete _store[key];
    }),
  };
  return { _store, storageLocal };
});

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    storage: { local: storageLocal },
  },
}));

// ── helpers ───────────────────────────────────────────────────────────────────

/** Expose the storage.local mock from the mocked browser namespace for per-test control. */
function getStorageLocalMock() {
  return (browser as unknown as { storage: { local: typeof storageLocal } }).storage.local;
}

/** A valid 64-char hex token. */
const VALID_TOKEN = 'f'.repeat(64);

function resetStorage(): void {
  for (const key of Object.keys(_store)) {
    delete _store[key];
  }
  vi.clearAllMocks();
  // Re-wire default implementations after clearAllMocks resets call histories.
  const local = getStorageLocalMock();
  local.get.mockImplementation(async (key: string) => ({ [key]: _store[key] }));
  local.set.mockImplementation(async (items: Record<string, unknown>) => {
    Object.assign(_store, items);
  });
  local.remove.mockImplementation(async (key: string) => {
    delete _store[key];
  });
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('storage – token round-trip (persist → load)', () => {
  beforeEach(resetStorage);

  it('setToken stores the trimmed value and getToken returns it', async () => {
    const stored = await setToken(`  ${VALID_TOKEN}  `);
    expect(stored).toBe(VALID_TOKEN);

    const loaded = await getToken();
    expect(loaded).toBe(VALID_TOKEN);
  });

  it('getToken returns null when nothing has been stored', async () => {
    const result = await getToken();
    expect(result).toBeNull();
  });

  it('clearToken removes the stored token and getToken returns null afterwards', async () => {
    await setToken(VALID_TOKEN);
    await clearToken();
    const result = await getToken();
    expect(result).toBeNull();
  });

  it('overwrites an existing token when setToken is called twice', async () => {
    const first = 'a'.repeat(64);
    const second = 'b'.repeat(64);
    await setToken(first);
    await setToken(second);
    expect(await getToken()).toBe(second);
  });
});

describe('storage – malformed / missing stored value shape check', () => {
  beforeEach(resetStorage);

  it('returns null (does not throw) when stored value is an empty string', async () => {
    getStorageLocalMock().get.mockResolvedValueOnce({ pairingToken: '' });
    const result = await getToken();
    expect(result).toBeNull();
  });

  it('returns null (does not throw) when stored value is a non-string type (e.g. number)', async () => {
    getStorageLocalMock().get.mockResolvedValueOnce({ pairingToken: 42 });
    const result = await getToken();
    expect(result).toBeNull();
  });

  it('returns null (does not throw) when stored value is null', async () => {
    getStorageLocalMock().get.mockResolvedValueOnce({ pairingToken: null });
    const result = await getToken();
    expect(result).toBeNull();
  });

  it('returns null (does not throw) when the key is absent from storage', async () => {
    getStorageLocalMock().get.mockResolvedValueOnce({});
    const result = await getToken();
    expect(result).toBeNull();
  });
});

describe('storage – answer-tools expand/collapse preference (UI boolean, not PII/job data)', () => {
  beforeEach(resetStorage);

  it('defaults to false (collapsed) when nothing has been stored', async () => {
    expect(await getAnswerToolsExpanded()).toBe(false);
  });

  it('round-trips true', async () => {
    await setAnswerToolsExpanded(true);
    expect(await getAnswerToolsExpanded()).toBe(true);
  });

  it('round-trips back to false after being set true', async () => {
    await setAnswerToolsExpanded(true);
    await setAnswerToolsExpanded(false);
    expect(await getAnswerToolsExpanded()).toBe(false);
  });

  it('defaults to false (does not throw) for a malformed stored value', async () => {
    getStorageLocalMock().get.mockResolvedValueOnce({ answerToolsExpanded: 'yes' });
    expect(await getAnswerToolsExpanded()).toBe(false);
  });
});

describe('looksLikeToken – input validation', () => {
  it('accepts a valid 64-char lowercase hex string', () => {
    expect(looksLikeToken(VALID_TOKEN)).toBe(true);
  });

  it('accepts a valid token with leading/trailing whitespace (trimmed internally)', () => {
    expect(looksLikeToken(`  ${VALID_TOKEN}  `)).toBe(true);
  });

  it('rejects a 63-char string (too short)', () => {
    expect(looksLikeToken('a'.repeat(63))).toBe(false);
  });

  it('rejects a 65-char string (too long)', () => {
    expect(looksLikeToken('a'.repeat(65))).toBe(false);
  });

  it('rejects uppercase hex characters', () => {
    expect(looksLikeToken('A'.repeat(64))).toBe(false);
  });

  it('rejects a string with non-hex characters', () => {
    expect(looksLikeToken('g'.repeat(64))).toBe(false);
  });

  it('rejects an empty string', () => {
    expect(looksLikeToken('')).toBe(false);
  });
});
