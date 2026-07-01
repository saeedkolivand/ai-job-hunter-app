/**
 * preferences-schema — custom performance mode tests.
 *
 * Covers:
 *  1. PerformanceModeSchema accepts 'custom'.
 *  2. resolveProfile: presets → PERFORMANCE_PRESETS; custom → customPerformance
 *     (with balanced fallback when undefined).
 *  3. Preset fidelity contract (regression-critical): exact field values for all
 *     three preset modes.
 *  4. resolveBackendConfig: tier→number tables (concurrency, keepAlive, cache TTL,
 *     cache max-rows) for every PerfTier, plus mode passthrough.
 */
import { describe, expect, it } from 'vitest';

import {
  PERFORMANCE_PRESETS,
  PerformanceModeSchema,
  type PerformanceProfile,
  resolveBackendConfig,
  resolveProfile,
} from './preferences-schema';

// ── 1. Schema accepts 'custom' ────────────────────────────────────────────────

describe("PerformanceModeSchema accepts 'custom'", () => {
  it("parses 'custom' without throwing", () => {
    expect(PerformanceModeSchema.parse('custom')).toBe('custom');
  });

  it('still rejects unknown values', () => {
    expect(() => PerformanceModeSchema.parse('turbo')).toThrow();
    expect(() => PerformanceModeSchema.parse('maximum')).toThrow();
  });

  it('accepts all four valid modes', () => {
    for (const mode of ['low-memory', 'balanced', 'performance', 'custom'] as const) {
      expect(PerformanceModeSchema.parse(mode)).toBe(mode);
    }
  });
});

// ── 2. resolveProfile ─────────────────────────────────────────────────────────

describe('resolveProfile', () => {
  it('returns the low-memory preset for performanceMode=low-memory', () => {
    const result = resolveProfile({ performanceMode: 'low-memory', customPerformance: undefined });
    expect(result).toBe(PERFORMANCE_PRESETS['low-memory']);
  });

  it('returns the balanced preset for performanceMode=balanced', () => {
    const result = resolveProfile({ performanceMode: 'balanced', customPerformance: undefined });
    expect(result).toBe(PERFORMANCE_PRESETS.balanced);
  });

  it('returns the performance preset for performanceMode=performance', () => {
    const result = resolveProfile({ performanceMode: 'performance', customPerformance: undefined });
    expect(result).toBe(PERFORMANCE_PRESETS.performance);
  });

  it('returns customPerformance when mode=custom and a custom profile exists', () => {
    const custom: PerformanceProfile = {
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
    const result = resolveProfile({ performanceMode: 'custom', customPerformance: custom });
    expect(result).toBe(custom);
  });

  it('falls back to the balanced preset when mode=custom but customPerformance is undefined', () => {
    const result = resolveProfile({ performanceMode: 'custom', customPerformance: undefined });
    expect(result).toBe(PERFORMANCE_PRESETS.balanced);
  });
});

// ── 3. Preset fidelity contract ───────────────────────────────────────────────

describe('PERFORMANCE_PRESETS fidelity contract', () => {
  describe("'low-memory' preset", () => {
    const p = PERFORMANCE_PRESETS['low-memory'];

    it('has all visual flags off', () => {
      expect(p.visual.aurora).toBe(false);
      expect(p.visual.nebula).toBe(false);
      expect(p.visual.richNebula).toBe(false);
      expect(p.visual.cursorGlow).toBe(false);
      expect(p.visual.animations).toBe(false);
    });

    it("has blur='reduced'", () => {
      expect(p.visual.blur).toBe('reduced');
    });

    it("has all backend tiers set to 'low'", () => {
      expect(p.backend.concurrency).toBe('low');
      expect(p.backend.keepAlive).toBe('low');
      expect(p.backend.cache).toBe('low');
    });
  });

  describe("'balanced' preset", () => {
    const p = PERFORMANCE_PRESETS.balanced;

    it('has aurora, nebula, cursorGlow, animations on; richNebula off', () => {
      expect(p.visual.aurora).toBe(true);
      expect(p.visual.nebula).toBe(true);
      expect(p.visual.richNebula).toBe(false);
      expect(p.visual.cursorGlow).toBe(true);
      expect(p.visual.animations).toBe(true);
    });

    it("has blur='full'", () => {
      expect(p.visual.blur).toBe('full');
    });

    it("has all backend tiers set to 'balanced'", () => {
      expect(p.backend.concurrency).toBe('balanced');
      expect(p.backend.keepAlive).toBe('balanced');
      expect(p.backend.cache).toBe('balanced');
    });
  });

  describe("'performance' preset", () => {
    const p = PERFORMANCE_PRESETS.performance;

    it('has aurora, nebula, richNebula, cursorGlow, animations all on', () => {
      expect(p.visual.aurora).toBe(true);
      expect(p.visual.nebula).toBe(true);
      expect(p.visual.richNebula).toBe(true);
      expect(p.visual.cursorGlow).toBe(true);
      expect(p.visual.animations).toBe(true);
    });

    it("has blur='full'", () => {
      expect(p.visual.blur).toBe('full');
    });

    it("has all backend tiers set to 'high'", () => {
      expect(p.backend.concurrency).toBe('high');
      expect(p.backend.keepAlive).toBe('high');
      expect(p.backend.cache).toBe('high');
    });
  });
});

// ── 4. resolveBackendConfig tier→number tables ────────────────────────────────

describe('resolveBackendConfig', () => {
  function profileWithTiers(
    concurrency: PerformanceProfile['backend']['concurrency'],
    keepAlive: PerformanceProfile['backend']['keepAlive'],
    cache: PerformanceProfile['backend']['cache']
  ): PerformanceProfile {
    return {
      visual: {
        aurora: false,
        nebula: false,
        richNebula: false,
        cursorGlow: false,
        blur: 'full',
        animations: true,
      },
      backend: { concurrency, keepAlive, cache },
    };
  }

  describe('concurrency tier→number', () => {
    it('low → 1', () => {
      const cfg = resolveBackendConfig('balanced', profileWithTiers('low', 'balanced', 'balanced'));
      expect(cfg.concurrency).toBe(1);
    });

    it('balanced → 2', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'balanced')
      );
      expect(cfg.concurrency).toBe(2);
    });

    it('high → 4', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('high', 'balanced', 'balanced')
      );
      expect(cfg.concurrency).toBe(4);
    });
  });

  describe('keepAliveSecs tier→number', () => {
    it('low → 0', () => {
      const cfg = resolveBackendConfig('balanced', profileWithTiers('balanced', 'low', 'balanced'));
      expect(cfg.keepAliveSecs).toBe(0);
    });

    it('balanced → 300', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'balanced')
      );
      expect(cfg.keepAliveSecs).toBe(300);
    });

    it('high → 1800', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'high', 'balanced')
      );
      expect(cfg.keepAliveSecs).toBe(1800);
    });
  });

  describe('cacheTtlSecs tier→number', () => {
    it('low → 86400 (1 day)', () => {
      const cfg = resolveBackendConfig('balanced', profileWithTiers('balanced', 'balanced', 'low'));
      expect(cfg.cacheTtlSecs).toBe(86400);
    });

    it('balanced → 604800 (7 days)', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'balanced')
      );
      expect(cfg.cacheTtlSecs).toBe(604800);
    });

    it('high → null (no expiry)', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'high')
      );
      expect(cfg.cacheTtlSecs).toBeNull();
    });
  });

  describe('cacheMaxRows tier→number', () => {
    it('low → 250', () => {
      const cfg = resolveBackendConfig('balanced', profileWithTiers('balanced', 'balanced', 'low'));
      expect(cfg.cacheMaxRows).toBe(250);
    });

    it('balanced → 2000', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'balanced')
      );
      expect(cfg.cacheMaxRows).toBe(2000);
    });

    it('high → null (unbounded)', () => {
      const cfg = resolveBackendConfig(
        'balanced',
        profileWithTiers('balanced', 'balanced', 'high')
      );
      expect(cfg.cacheMaxRows).toBeNull();
    });
  });

  describe('mode passthrough', () => {
    it('passes the mode string through to the config for each mode', () => {
      const profile = profileWithTiers('balanced', 'balanced', 'balanced');
      expect(resolveBackendConfig('low-memory', profile).mode).toBe('low-memory');
      expect(resolveBackendConfig('balanced', profile).mode).toBe('balanced');
      expect(resolveBackendConfig('performance', profile).mode).toBe('performance');
      expect(resolveBackendConfig('custom', profile).mode).toBe('custom');
    });
  });

  describe('custom mode end-to-end', () => {
    it('correctly resolves a custom profile with mixed tiers', () => {
      const custom: PerformanceProfile = {
        visual: {
          aurora: true,
          nebula: true,
          richNebula: false,
          cursorGlow: false,
          blur: 'reduced',
          animations: true,
        },
        backend: { concurrency: 'high', keepAlive: 'low', cache: 'balanced' },
      };
      const cfg = resolveBackendConfig('custom', custom);
      expect(cfg.mode).toBe('custom');
      expect(cfg.concurrency).toBe(4); // high
      expect(cfg.keepAliveSecs).toBe(0); // low
      expect(cfg.cacheTtlSecs).toBe(604800); // balanced
      expect(cfg.cacheMaxRows).toBe(2000); // balanced
    });
  });
});
