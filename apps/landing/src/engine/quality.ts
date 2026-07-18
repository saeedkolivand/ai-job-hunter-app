// Quality tier resolution: a one-shot device probe picked at first client call
// and cached module-level so every consumer reads one consistent tier for the
// session. Mirrors gate.ts: no window access at module scope, computed lazily.
// HIGH is the safe SSR default (returned uncached so the first real client call
// still probes). Only the boil rate is a hard per-tier constant M1 consumes
// (uBoil steps at boilHz); dpr is exposed for later tiering but the Canvas
// itself clamps devicePixelRatio to [1,2] per the M1 brief.

export interface Tier {
  dpr: number;
  boilHz: number;
}

export const TIER_TABLE: { HIGH: Tier; LOW: Tier } = {
  HIGH: { dpr: 2, boilHz: 10 },
  LOW: { dpr: 1.25, boilHz: 8 },
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
