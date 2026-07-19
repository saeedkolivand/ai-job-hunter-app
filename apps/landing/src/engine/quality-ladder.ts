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
}

const LADDER: Record<QualityTier, QualityLadder> = {
  HIGH: { stormCount: 4000, towerCount: 160 },
  MID: { stormCount: 2000, towerCount: 96 },
  LOW: { stormCount: 900, towerCount: 56 },
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
