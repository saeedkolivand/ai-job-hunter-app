import { describe, expect, it } from 'vitest';

import { suggestLocalLimits } from './suggest-local-limits';

describe('suggestLocalLimits', () => {
  it('caps the context window at the model max on a high-memory machine', () => {
    const s = suggestLocalLimits({
      modelMaxContext: 8192,
      freeRamGb: 64,
      hasGpu: false,
      freeVramGb: 0,
    });
    expect(s.contextWindow).toBe(8192); // never exceeds the model's trained max
  });

  it('drops the context window on a low-RAM machine', () => {
    const low = suggestLocalLimits({
      modelMaxContext: 131072,
      freeRamGb: 2,
      hasGpu: false,
      freeVramGb: 0,
    });
    const high = suggestLocalLimits({
      modelMaxContext: 131072,
      freeRamGb: 64,
      hasGpu: false,
      freeVramGb: 0,
    });
    expect(low.contextWindow).toBeLessThan(high.contextWindow);
    expect(low.contextWindow).toBeGreaterThanOrEqual(2048); // never below the floor
  });

  it('budgets against VRAM when a GPU is present', () => {
    // Lots of RAM but tiny VRAM → the GPU budget governs and keeps it small.
    const s = suggestLocalLimits({
      modelMaxContext: 131072,
      freeRamGb: 64,
      hasGpu: true,
      freeVramGb: 2,
    });
    expect(s.contextWindow).toBeLessThan(32768);
  });

  it('stays within the schema bounds and rounds to a 512-token step', () => {
    const s = suggestLocalLimits({
      modelMaxContext: 200000, // beyond the schema ceiling
      freeRamGb: 1024,
      hasGpu: false,
      freeVramGb: 0,
    });
    expect(s.contextWindow).toBeLessThanOrEqual(131072);
    expect(s.contextWindow % 512).toBe(0);
    expect(s.maxTokens).toBeGreaterThanOrEqual(512);
    expect(s.maxTokens).toBeLessThanOrEqual(8192);
  });

  it('falls back to a safe default when the model max is unknown', () => {
    const s = suggestLocalLimits({ freeRamGb: 16, hasGpu: false, freeVramGb: 0 });
    expect(s.contextWindow).toBeGreaterThanOrEqual(2048);
    expect(s.contextWindow).toBeLessThanOrEqual(131072);
  });
});
