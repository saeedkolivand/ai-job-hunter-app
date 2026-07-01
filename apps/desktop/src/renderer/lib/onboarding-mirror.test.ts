/**
 * onboarding-mirror — unit tests
 *
 * Strategy:
 *  - @tauri-apps/plugin-store is mocked: Store.load returns a fake store with
 *    get/set/delete/save spies so no Tauri runtime is needed.
 *  - vi.hoisted() is used to declare spies before vi.mock() factory hoisting.
 *  - mockStoreLoad is an explicit spy so individual tests can override it with
 *    mockRejectedValueOnce to exercise the try/catch error paths.
 *
 * Coverage:
 *  1. markOnboardingComplete → set(KEY, true) then save().
 *  2. readOnboardingComplete → returns true only for exact boolean true.
 *  3. clearOnboardingMirror → delete(KEY) then save().
 *  4. Error paths — Store.load or store.save rejects → functions resolve without throwing.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

// ── hoisted spies (must be declared before vi.mock is hoisted) ────────────────

const { mockGet, mockSet, mockDelete, mockSave, mockStoreLoad } = vi.hoisted(() => {
  const mockGet = vi.fn().mockResolvedValue(undefined);
  const mockSet = vi.fn().mockResolvedValue(undefined);
  const mockDelete = vi.fn().mockResolvedValue(undefined);
  const mockSave = vi.fn().mockResolvedValue(undefined);
  const fakeStore = { get: mockGet, set: mockSet, delete: mockDelete, save: mockSave };
  const mockStoreLoad = vi.fn().mockResolvedValue(fakeStore);
  return { mockGet, mockSet, mockDelete, mockSave, mockStoreLoad };
});

vi.mock('@tauri-apps/plugin-store', () => ({
  Store: { load: mockStoreLoad },
}));

// ── import after mock ─────────────────────────────────────────────────────────

import {
  clearOnboardingMirror,
  markOnboardingComplete,
  readOnboardingComplete,
} from './onboarding-mirror';

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockGet.mockReset();
  mockGet.mockResolvedValue(undefined);
  mockSet.mockReset();
  mockSet.mockResolvedValue(undefined);
  mockDelete.mockReset();
  mockDelete.mockResolvedValue(undefined);
  mockSave.mockReset();
  mockSave.mockResolvedValue(undefined);
  mockStoreLoad.mockReset();
  mockStoreLoad.mockResolvedValue({
    get: mockGet,
    set: mockSet,
    delete: mockDelete,
    save: mockSave,
  });
});

// ── markOnboardingComplete ────────────────────────────────────────────────────

describe('markOnboardingComplete', () => {
  it('calls set("onboardingCompleted", true) then save()', async () => {
    const callOrder: string[] = [];
    mockSet.mockImplementation(async () => {
      callOrder.push('set');
    });
    mockSave.mockImplementation(async () => {
      callOrder.push('save');
    });

    await markOnboardingComplete();

    expect(mockSet).toHaveBeenCalledWith('onboardingCompleted', true);
    expect(mockSave).toHaveBeenCalledOnce();
    expect(callOrder).toEqual(['set', 'save']);
  });
});

// ── readOnboardingComplete ────────────────────────────────────────────────────

describe('readOnboardingComplete', () => {
  it('returns true when stored value is exactly true', async () => {
    mockGet.mockResolvedValue(true);
    expect(await readOnboardingComplete()).toBe(true);
  });

  it('returns false when stored value is undefined', async () => {
    mockGet.mockResolvedValue(undefined);
    expect(await readOnboardingComplete()).toBe(false);
  });

  it('returns false when stored value is null', async () => {
    mockGet.mockResolvedValue(null);
    expect(await readOnboardingComplete()).toBe(false);
  });

  it('returns false when stored value is false', async () => {
    mockGet.mockResolvedValue(false);
    expect(await readOnboardingComplete()).toBe(false);
  });

  it('returns false when stored value is 1 (truthy but not exactly true)', async () => {
    mockGet.mockResolvedValue(1);
    expect(await readOnboardingComplete()).toBe(false);
  });
});

// ── clearOnboardingMirror ─────────────────────────────────────────────────────

describe('clearOnboardingMirror', () => {
  it('calls delete("onboardingCompleted") then save()', async () => {
    const callOrder: string[] = [];
    mockDelete.mockImplementation(async () => {
      callOrder.push('delete');
    });
    mockSave.mockImplementation(async () => {
      callOrder.push('save');
    });

    await clearOnboardingMirror();

    expect(mockDelete).toHaveBeenCalledWith('onboardingCompleted');
    expect(mockSave).toHaveBeenCalledOnce();
    expect(callOrder).toEqual(['delete', 'save']);
  });
});

// ── Error paths — best-effort: failures must not throw ───────────────────────
//
// All three public functions wrap their bodies in try/catch so that a missing
// Tauri store (e.g. first boot, FS error) is non-fatal to the UI.

describe('onboarding-mirror — error paths (best-effort / non-throwing)', () => {
  it('Store.load rejects → readOnboardingComplete resolves to false (never throws)', async () => {
    mockStoreLoad.mockRejectedValueOnce(new Error('fs error'));
    await expect(readOnboardingComplete()).resolves.toBe(false);
  });

  it('Store.load rejects → markOnboardingComplete resolves without throwing', async () => {
    mockStoreLoad.mockRejectedValueOnce(new Error('fs error'));
    await expect(markOnboardingComplete()).resolves.toBeUndefined();
  });

  it('store.save rejects → markOnboardingComplete resolves without throwing', async () => {
    mockSave.mockRejectedValueOnce(new Error('disk full'));
    await expect(markOnboardingComplete()).resolves.toBeUndefined();
  });

  it('Store.load rejects → clearOnboardingMirror resolves without throwing', async () => {
    mockStoreLoad.mockRejectedValueOnce(new Error('fs error'));
    await expect(clearOnboardingMirror()).resolves.toBeUndefined();
  });

  it('store.save rejects → clearOnboardingMirror resolves without throwing', async () => {
    mockSave.mockRejectedValueOnce(new Error('disk full'));
    await expect(clearOnboardingMirror()).resolves.toBeUndefined();
  });
});
