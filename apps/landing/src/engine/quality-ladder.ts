// The per-tier quality ladder (skill's "Budgets + quality governor" table).
// Pure data + selectors, no runtime deps -- the storm/tower instance counts a GL
// scene asks for, keyed by the store's QualityTier. The governor moves BETWEEN
// these rungs (pixel ratio -> post samples -> geometry density -> effect
// toggles); it never invents new ones. dpr/MSAA live with the gate (dprCap);
// this module owns the geometry-density rungs.

import type { QualityTier } from "./store";

export interface QualityLadder {
  // Paper-storm InstancedMesh instance budget (skill ladder: 4000 / 2000 / 900).
  readonly stormCount: number;
  // Tower InstancedMesh instance budget -- one draw call regardless of count, so
  // this is a vertex/overdraw knob, not a draw-call one. Author-chosen within the
  // ADR envelope (not a fixed skill figure).
  readonly towerCount: number;
  // M3 water/deep density knobs (all one draw call each except the god-ray
  // shafts, which are a handful of separate additive cones). Author-chosen within
  // the ADR envelope -- geometry-density rungs the governor moves between.
  readonly waterSegments: number; // Gerstner patch subdivisions per side
  readonly surfaceLetterCount: number; // floating letters paving the surface
  readonly deepPaperCount: number; // sparse drifting papers in the deep
  readonly godrayShaftCount: number; // additive god-ray cones (draw calls: this many)
}

// godrayShaftCount lowered (M3 review fix, alongside the shaft radius/strength
// retune in water-layout.ts): fewer, thinner, dimmer shafts so additive
// overlap can't stack back up into the overblown wash the coordinator flagged.
const LADDER: Record<QualityTier, QualityLadder> = {
  HIGH: {
    stormCount: 4000,
    towerCount: 160,
    waterSegments: 96,
    surfaceLetterCount: 140,
    deepPaperCount: 70,
    godrayShaftCount: 4,
  },
  MID: {
    stormCount: 2000,
    towerCount: 96,
    waterSegments: 64,
    surfaceLetterCount: 90,
    deepPaperCount: 44,
    godrayShaftCount: 3,
  },
  LOW: {
    stormCount: 900,
    towerCount: 56,
    waterSegments: 40,
    surfaceLetterCount: 48,
    deepPaperCount: 24,
    godrayShaftCount: 2,
  },
};

export function qualityLadder(tier: QualityTier): QualityLadder {
  return LADDER[tier];
}

export function stormCountForTier(tier: QualityTier): number {
  return LADDER[tier].stormCount;
}

export function towerCountForTier(tier: QualityTier): number {
  return LADDER[tier].towerCount;
}

export function waterSegmentsForTier(tier: QualityTier): number {
  return LADDER[tier].waterSegments;
}

export function surfaceLetterCountForTier(tier: QualityTier): number {
  return LADDER[tier].surfaceLetterCount;
}

export function deepPaperCountForTier(tier: QualityTier): number {
  return LADDER[tier].deepPaperCount;
}

export function godrayShaftCountForTier(tier: QualityTier): number {
  return LADDER[tier].godrayShaftCount;
}
