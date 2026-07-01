/**
 * useWindowControls — unit tests
 *
 * Strategy:
 *  - @tauri-apps/api/window is mocked: getCurrentWindow() returns a spy object
 *    so no real Tauri window is needed.
 *  - @tauri-apps/plugin-os is mocked: platform() returns a controlled string.
 *  - @tauri-apps/api/app and @tauri-apps/plugin-positioner are mocked as no-ops
 *    (not under test here).
 *  - vi.hoisted() is used for all spies referenced inside vi.mock() factories
 *    to avoid TDZ errors after hoisting.
 *
 * Coverage:
 *  1. setTaskbarProgress mapping — the primary business logic.
 *  2. isMacos derives correctly from mocked platform().
 *  3. foreground() calls unminimize then setFocus in order.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

// ── hoisted spies ─────────────────────────────────────────────────────────────

const {
  mockSetProgressBar,
  mockUnminimize,
  mockSetFocus,
  mockToggleMaximize,
  mockIsFocused,
  mockRequestUserAttention,
} = vi.hoisted(() => ({
  mockSetProgressBar: vi.fn().mockResolvedValue(undefined),
  mockUnminimize: vi.fn().mockResolvedValue(undefined),
  mockSetFocus: vi.fn().mockResolvedValue(undefined),
  mockToggleMaximize: vi.fn().mockResolvedValue(undefined),
  mockIsFocused: vi.fn().mockResolvedValue(true),
  mockRequestUserAttention: vi.fn().mockResolvedValue(undefined),
}));

// Mutable container for platform so it can be changed per-test.
const platformContainer = { value: 'windows' };

// ── @tauri-apps/api/window ─────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    setProgressBar: mockSetProgressBar,
    unminimize: mockUnminimize,
    setFocus: mockSetFocus,
    toggleMaximize: mockToggleMaximize,
    isFocused: mockIsFocused,
    requestUserAttention: mockRequestUserAttention,
  }),
  ProgressBarStatus: {
    None: 'none',
    Indeterminate: 'indeterminate',
    Normal: 'normal',
  },
  UserAttentionType: { Informational: 'informational' },
}));

// ── @tauri-apps/plugin-os ──────────────────────────────────────────────────────

vi.mock('@tauri-apps/plugin-os', () => ({
  platform: () => platformContainer.value,
}));

// ── @tauri-apps/api/app — not under test ──────────────────────────────────────

vi.mock('@tauri-apps/api/app', () => ({
  hide: vi.fn().mockResolvedValue(undefined),
  show: vi.fn().mockResolvedValue(undefined),
}));

// ── @tauri-apps/plugin-positioner — not under test ────────────────────────────

vi.mock('@tauri-apps/plugin-positioner', () => ({
  moveWindow: vi.fn().mockResolvedValue(undefined),
  Position: { Center: 'center' },
}));

// ── import after mocks ─────────────────────────────────────────────────────────

import { useWindowControls } from './use-window-controls';

// ── helpers ───────────────────────────────────────────────────────────────────

function getControls() {
  const { result } = renderHook(() => useWindowControls());
  return result.current;
}

// ── Tauri runtime sentinel ─────────────────────────────────────────────────────
// The runtime check lives inside useMemo, so setting __TAURI_INTERNALS__ before
// each render is enough to route tests to the real branch. The no-op branch test
// simply deletes it before calling renderHook.

beforeEach(() => {
  (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__ = {};
  mockSetProgressBar.mockClear();
  mockUnminimize.mockClear();
  mockSetFocus.mockClear();
  platformContainer.value = 'windows';
});

afterEach(() => {
  delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
});

// ── setTaskbarProgress ────────────────────────────────────────────────────────

describe('useWindowControls — setTaskbarProgress', () => {
  it('null → ProgressBarStatus.None', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(null);
    expect(mockSetProgressBar).toHaveBeenCalledOnce();
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'none' });
  });

  it('negative number (-1) → ProgressBarStatus.Indeterminate', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(-1);
    expect(mockSetProgressBar).toHaveBeenCalledOnce();
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'indeterminate' });
  });

  it('0.5 → ProgressBarStatus.Normal with progress: 50', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(0.5);
    expect(mockSetProgressBar).toHaveBeenCalledOnce();
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'normal', progress: 50 });
  });

  it('boundary 0 → Normal with progress: 0', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(0);
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'normal', progress: 0 });
  });

  it('boundary 1 → Normal with progress: 100', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(1);
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'normal', progress: 100 });
  });

  it('rounds fractional progress — 0.336 → 34', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(0.336);
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'normal', progress: 34 });
  });

  it('clamps value > 1 to progress: 100', async () => {
    const { setTaskbarProgress } = getControls();
    await setTaskbarProgress(1.5);
    expect(mockSetProgressBar).toHaveBeenCalledWith({ status: 'normal', progress: 100 });
  });
});

// ── isMacos ───────────────────────────────────────────────────────────────────

describe('useWindowControls — isMacos', () => {
  it('returns true when platform() is "macos"', () => {
    platformContainer.value = 'macos';
    const { isMacos } = getControls();
    expect(isMacos).toBe(true);
  });

  it('returns false when platform() is "windows"', () => {
    platformContainer.value = 'windows';
    const { isMacos } = getControls();
    expect(isMacos).toBe(false);
  });

  it('returns false when platform() is "linux"', () => {
    platformContainer.value = 'linux';
    const { isMacos } = getControls();
    expect(isMacos).toBe(false);
  });
});

// ── foreground() call order ────────────────────────────────────────────────────

describe('useWindowControls — foreground', () => {
  it('calls unminimize then setFocus in order', async () => {
    const callOrder: string[] = [];
    mockUnminimize.mockImplementation(async () => {
      callOrder.push('unminimize');
    });
    mockSetFocus.mockImplementation(async () => {
      callOrder.push('setFocus');
    });

    const { foreground } = getControls();
    await foreground();

    expect(callOrder).toEqual(['unminimize', 'setFocus']);
  });
});

// ── no-op branch (browser / E2E harness — no Tauri runtime) ──────────────────
// The Tauri check lives inside useMemo, so deleting __TAURI_INTERNALS__ before
// renderHook is enough — no module reset needed.

describe('useWindowControls — no-op branch (no Tauri runtime)', () => {
  it('returns isMacos=false, isFocused resolves true, and fire-and-forget methods resolve without calling mocked getCurrentWindow', async () => {
    // beforeEach sets __TAURI_INTERNALS__ = {}; delete it here so the
    // runtime check inside useMemo takes the no-op path for this test.
    delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;

    const { result } = renderHook(() => useWindowControls());
    const controls = result.current;

    expect(controls.isMacos).toBe(false);
    await expect(controls.isFocused()).resolves.toBe(true);
    await expect(controls.setTaskbarProgress(0.5)).resolves.toBeUndefined();
    await expect(controls.toggleMaximize()).resolves.toBeUndefined();

    // getCurrentWindow was never reached — setProgressBar must not have been called.
    expect(mockSetProgressBar).not.toHaveBeenCalled();
  });
});
