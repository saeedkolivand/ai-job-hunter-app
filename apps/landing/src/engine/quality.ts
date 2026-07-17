// Quality tier resolution: one-shot device probe that picks HIGH or LOW at
// gate time and caches the result module-level so every consumer (dpr, the
// line-boil clock, the stroke budget) reads one consistent tier for the whole
// session. Mirrors gate.ts: no window access at module scope, computed lazily
// on the first client call. HIGH is the safe default under SSR (returned
// uncached so the first real client call still probes and caches).
//
// LOW when the device looks weak -- devicePixelRatio < 1.5 (no retina) OR
// hardwareConcurrency <= 4 (few cores) -- else HIGH. See TIER_TABLE.

export interface Tier {
  dpr: number;
  boilFps: number;
  strokeBudget: number;
}

// The webgl-standards quality-tier table, in code. strokeBudget is a scale:
// 1 = full, 0.5 = halved.
export const TIER_TABLE: { HIGH: Tier; LOW: Tier } = {
  HIGH: { dpr: 2, boilFps: 10, strokeBudget: 1 },
  LOW: { dpr: 1.25, boilFps: 8, strokeBudget: 0.5 },
};

let cached: Tier | null = null;

function probe(): Tier {
  const lowDpr = (window.devicePixelRatio || 1) < 1.5;
  const fewCores = (navigator.hardwareConcurrency || 1) <= 4;
  return lowDpr || fewCores ? TIER_TABLE.LOW : TIER_TABLE.HIGH;
}

export function resolveTier(): Tier {
  if (typeof window === "undefined") return TIER_TABLE.HIGH;
  if (cached === null) cached = probe();
  return cached;
}
