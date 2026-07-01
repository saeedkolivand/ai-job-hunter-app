/**
 * PerformanceModeProvider — custom performance mode integration tests.
 *
 * Covers:
 *  1. setPerformanceMode IPC is called with the resolved PerformanceBackendConfig
 *     (correct numbers) for each mode.
 *  2. <html> receives data-performance-mode, data-perf-blur, data-perf-animations.
 *  3. A visual-only change (same backend tiers) does NOT re-invoke IPC (dedupe guard).
 *  4. A backend/mode change DOES re-invoke IPC.
 *
 * Strategy:
 *  - Mock the preferences-store selectors so tests can inject any profile without
 *    a real store; this sidesteps Zustand + localStorage setup.
 *  - Use createMockClient from @/lib/mock-client (the real factory) so the IPC
 *    spy is a proper vi.fn().
 *  - afterEach: clean up data-* attributes on <html> so tests don't bleed.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';

import type { PerformanceBackendConfig } from '@ajh/shared';

import { createMockClient } from '@/lib/mock-client';
import {
  PERFORMANCE_PRESETS,
  type PerformanceMode,
  type PerformanceProfile,
  resolveBackendConfig,
} from '@/store/preferences-schema';

import { AppClientProvider } from '../AppClientProvider';
import { PerformanceModeProvider } from './PerformanceModeProvider';

// ── mock preferences-store selectors ─────────────────────────────────────────

let currentMode: PerformanceMode = 'balanced';
let currentProfile: PerformanceProfile = PERFORMANCE_PRESETS.balanced;

vi.mock('@/store/preferences-store', () => ({
  usePerformanceMode: () => currentMode,
  useResolvedPerformanceProfile: () => currentProfile,
}));

// ── helpers ───────────────────────────────────────────────────────────────────

function renderProvider(setPerformanceMode: ReturnType<typeof vi.fn>) {
  const client = createMockClient({ system: { setPerformanceMode } });
  return render(
    <AppClientProvider client={client}>
      <PerformanceModeProvider>
        <span>child</span>
      </PerformanceModeProvider>
    </AppClientProvider>
  );
}

// ── cleanup ───────────────────────────────────────────────────────────────────

afterEach(() => {
  const root = document.documentElement;
  root.removeAttribute('data-performance-mode');
  root.removeAttribute('data-perf-blur');
  root.removeAttribute('data-perf-animations');
  currentMode = 'balanced';
  currentProfile = PERFORMANCE_PRESETS.balanced;
});

// ── tests ─────────────────────────────────────────────────────────────────────

describe('PerformanceModeProvider — custom performance mode', () => {
  describe('IPC payload correctness', () => {
    it('calls setPerformanceMode with the balanced backend config on mount', async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
      const call = spy.mock.calls.at(0);
      if (!call) throw new Error('expected setPerformanceMode to have been called');
      const config = call[0] as PerformanceBackendConfig;
      expect(config.mode).toBe('balanced');
      expect(config.concurrency).toBe(2);
      expect(config.keepAliveSecs).toBe(300);
      expect(config.cacheTtlSecs).toBe(604800);
      expect(config.cacheMaxRows).toBe(2000);
    });

    it('calls setPerformanceMode with the low-memory backend config', async () => {
      currentMode = 'low-memory';
      currentProfile = PERFORMANCE_PRESETS['low-memory'];
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
      const call = spy.mock.calls.at(0);
      if (!call) throw new Error('expected setPerformanceMode to have been called');
      const config = call[0] as PerformanceBackendConfig;
      expect(config.mode).toBe('low-memory');
      expect(config.concurrency).toBe(1);
      expect(config.keepAliveSecs).toBe(0);
      expect(config.cacheTtlSecs).toBe(86400);
      expect(config.cacheMaxRows).toBe(250);
    });

    it('calls setPerformanceMode with the performance backend config', async () => {
      currentMode = 'performance';
      currentProfile = PERFORMANCE_PRESETS.performance;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
      const call = spy.mock.calls.at(0);
      if (!call) throw new Error('expected setPerformanceMode to have been called');
      const config = call[0] as PerformanceBackendConfig;
      expect(config.mode).toBe('performance');
      expect(config.concurrency).toBe(4);
      expect(config.keepAliveSecs).toBe(1800);
      expect(config.cacheTtlSecs).toBeNull();
      expect(config.cacheMaxRows).toBeNull();
    });

    it('calls setPerformanceMode with the custom backend config (mixed tiers)', async () => {
      const customProfile: PerformanceProfile = {
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
      currentMode = 'custom';
      currentProfile = customProfile;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
      const call = spy.mock.calls.at(0);
      if (!call) throw new Error('expected setPerformanceMode to have been called');
      const config = call[0] as PerformanceBackendConfig;
      expect(config.mode).toBe('custom');
      expect(config.concurrency).toBe(4); // high
      expect(config.keepAliveSecs).toBe(0); // low
      expect(config.cacheTtlSecs).toBe(604800); // balanced
      expect(config.cacheMaxRows).toBe(2000); // balanced
    });
  });

  describe('<html> data attributes', () => {
    it('sets data-performance-mode to the active mode', async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-performance-mode')).toBe('balanced');
    });

    it('sets data-perf-blur to the resolved blur tier', async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-perf-blur')).toBe('full');
    });

    it("sets data-perf-animations='on' when animations are enabled", async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-perf-animations')).toBe('on');
    });

    it("sets data-perf-animations='off' for the low-memory preset", async () => {
      currentMode = 'low-memory';
      currentProfile = PERFORMANCE_PRESETS['low-memory'];
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-perf-animations')).toBe('off');
    });

    it("sets data-perf-blur='reduced' for the low-memory preset", async () => {
      currentMode = 'low-memory';
      currentProfile = PERFORMANCE_PRESETS['low-memory'];
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-perf-blur')).toBe('reduced');
    });

    it("reflects 'off' blur tier for a custom profile with blur='off'", async () => {
      const customProfile: PerformanceProfile = {
        visual: {
          aurora: false,
          nebula: false,
          richNebula: false,
          cursorGlow: false,
          blur: 'off',
          animations: false,
        },
        backend: { concurrency: 'balanced', keepAlive: 'balanced', cache: 'balanced' },
      };
      currentMode = 'custom';
      currentProfile = customProfile;
      const spy = vi.fn().mockResolvedValue(undefined);
      renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(document.documentElement.getAttribute('data-perf-blur')).toBe('off');
      expect(document.documentElement.getAttribute('data-performance-mode')).toBe('custom');
    });
  });

  describe('IPC deduplication', () => {
    it('does NOT re-invoke IPC when only visual fields change (same backend config)', async () => {
      // Start with balanced.
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      const client = createMockClient({ system: { setPerformanceMode: spy } });

      const { rerender } = render(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));

      // Simulate a visual-only change: same mode + same backend tiers, blur flipped to
      // 'reduced'. The backend config JSON is unchanged → dedupe guard should suppress
      // the IPC call. Using a visual field that produces an OBSERVABLE attribute change
      // (data-perf-blur goes 'full' → 'reduced') proves the effect ran for this cycle.
      const visualOnlyChange: PerformanceProfile = {
        visual: { ...PERFORMANCE_PRESETS.balanced.visual, blur: 'reduced' },
        backend: PERFORMANCE_PRESETS.balanced.backend, // identical backend
      };
      currentMode = 'balanced';
      currentProfile = visualOnlyChange;

      // Rerender with the new profile. Do NOT use a raw setTimeout to settle
      // effects — that is non-deterministic under microtask scheduling. Instead,
      // waitFor on the NEW blur attribute value the effect WILL write on this cycle.
      // When the waitFor resolves, the effect has provably completed its DOM writes.
      rerender(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      // Wait until the effect has written the CHANGED blur attribute — this
      // guarantees the full effect body has run for the re-render cycle.
      await waitFor(() =>
        expect(document.documentElement.getAttribute('data-perf-blur')).toBe('reduced')
      );

      // blur is a visual.* field — resolveBackendConfig reads only profile.backend.* —
      // so the serialized backend config is identical → no second IPC call.
      expect(spy.mock.calls.length).toBe(1);
    });

    it('re-invokes IPC when the backend config changes (mode switch)', async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockResolvedValue(undefined);
      const client = createMockClient({ system: { setPerformanceMode: spy } });

      const { rerender } = render(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));

      // Switch to low-memory — different backend config → IPC must re-fire.
      currentMode = 'low-memory';
      currentProfile = PERFORMANCE_PRESETS['low-memory'];

      rerender(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
      const secondCall = spy.mock.calls.at(1);
      if (!secondCall) throw new Error('expected a second IPC call');
      const config = secondCall[0] as PerformanceBackendConfig;
      expect(config.mode).toBe('low-memory');
      expect(config.concurrency).toBe(1);
    });

    it('re-invokes IPC when custom backend tiers change', async () => {
      const customV1: PerformanceProfile = {
        visual: {
          aurora: true,
          nebula: true,
          richNebula: false,
          cursorGlow: true,
          blur: 'full',
          animations: true,
        },
        backend: { concurrency: 'high', keepAlive: 'low', cache: 'balanced' },
      };
      currentMode = 'custom';
      currentProfile = customV1;
      const spy = vi.fn().mockResolvedValue(undefined);
      const client = createMockClient({ system: { setPerformanceMode: spy } });

      const { rerender } = render(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));

      // Change only the keepAlive backend tier → config serialization changes.
      const customV2: PerformanceProfile = {
        ...customV1,
        backend: { ...customV1.backend, keepAlive: 'balanced' },
      };
      currentProfile = customV2;

      rerender(
        <AppClientProvider client={client}>
          <PerformanceModeProvider>
            <span>child</span>
          </PerformanceModeProvider>
        </AppClientProvider>
      );

      await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
      const secondCall = spy.mock.calls.at(1);
      if (!secondCall) throw new Error('expected a second IPC call');
      const config = secondCall[0] as PerformanceBackendConfig;
      expect(config.keepAliveSecs).toBe(300); // balanced
    });
  });

  describe('error resilience', () => {
    it('swallows backend errors without crashing the child', async () => {
      currentMode = 'balanced';
      currentProfile = PERFORMANCE_PRESETS.balanced;
      const spy = vi.fn().mockRejectedValue(new Error('not ready'));
      const { getByText } = renderProvider(spy);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expect(getByText('child')).toBeInTheDocument();
    });
  });
});

// Also re-export a basic sanity check to replace the existing test's coverage.
describe('PerformanceModeProvider — resolveBackendConfig sanity (from provider)', () => {
  it('resolveBackendConfig for balanced preset matches expected IPC payload', () => {
    const cfg = resolveBackendConfig('balanced', PERFORMANCE_PRESETS.balanced);
    expect(cfg).toEqual({
      mode: 'balanced',
      concurrency: 2,
      keepAliveSecs: 300,
      cacheTtlSecs: 604800,
      cacheMaxRows: 2000,
    });
  });
});
