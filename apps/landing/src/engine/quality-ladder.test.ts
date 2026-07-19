import { describe, expect, it } from "vitest";

import {
  deepPaperCountForTier,
  godrayShaftCountForTier,
  qualityLadder,
  stormCountForTier,
  surfaceLetterCountForTier,
  towerCountForTier,
  waterSegmentsForTier,
} from "./quality-ladder";
import type { QualityTier } from "./store";

const TIERS: QualityTier[] = ["HIGH", "MID", "LOW"];

describe("quality ladder", () => {
  it("matches the skill's paper-storm counts per tier", () => {
    expect(stormCountForTier("HIGH")).toBe(4000);
    expect(stormCountForTier("MID")).toBe(2000);
    expect(stormCountForTier("LOW")).toBe(900);
  });

  it("storm + tower counts descend monotonically HIGH -> MID -> LOW", () => {
    for (let i = 0; i < TIERS.length - 1; i++) {
      const hi = TIERS[i];
      const lo = TIERS[i + 1];
      if (!hi || !lo) continue;
      expect(stormCountForTier(hi)).toBeGreaterThan(stormCountForTier(lo));
      expect(towerCountForTier(hi)).toBeGreaterThan(towerCountForTier(lo));
    }
  });

  it("every tier yields positive, finite instance budgets", () => {
    for (const tier of TIERS) {
      const l = qualityLadder(tier);
      expect(Number.isInteger(l.stormCount)).toBe(true);
      expect(l.stormCount).toBeGreaterThan(0);
      expect(Number.isInteger(l.towerCount)).toBe(true);
      expect(l.towerCount).toBeGreaterThan(0);
    }
  });

  it("selectors agree with the ladder object", () => {
    for (const tier of TIERS) {
      expect(stormCountForTier(tier)).toBe(qualityLadder(tier).stormCount);
      expect(towerCountForTier(tier)).toBe(qualityLadder(tier).towerCount);
    }
  });

  it("M3 water/deep density knobs are positive integers and never rise HIGH -> LOW", () => {
    const selectors = [
      waterSegmentsForTier,
      surfaceLetterCountForTier,
      deepPaperCountForTier,
      godrayShaftCountForTier,
    ];
    for (const sel of selectors) {
      for (const tier of TIERS) {
        expect(Number.isInteger(sel(tier))).toBe(true);
        expect(sel(tier)).toBeGreaterThan(0);
      }
      for (let i = 0; i < TIERS.length - 1; i++) {
        const hi = TIERS[i];
        const lo = TIERS[i + 1];
        if (!hi || !lo) continue;
        expect(sel(hi)).toBeGreaterThanOrEqual(sel(lo));
      }
    }
  });

  it("M3 selectors agree with the ladder object", () => {
    for (const tier of TIERS) {
      const l = qualityLadder(tier);
      expect(waterSegmentsForTier(tier)).toBe(l.waterSegments);
      expect(surfaceLetterCountForTier(tier)).toBe(l.surfaceLetterCount);
      expect(deepPaperCountForTier(tier)).toBe(l.deepPaperCount);
      expect(godrayShaftCountForTier(tier)).toBe(l.godrayShaftCount);
    }
  });
});
