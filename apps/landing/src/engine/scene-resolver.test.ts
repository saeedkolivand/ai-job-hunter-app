import { describe, expect, it } from "vitest";

import { resolveScene, sceneById, sceneProgress, SCENES, sceneStartT } from "./scene-resolver";

describe("SCENES table", () => {
  it("covers [0,1] with 9 contiguous, non-overlapping half-open intervals", () => {
    expect(SCENES).toHaveLength(9);
    expect(SCENES[0]?.lo).toBe(0);
    expect(SCENES[8]?.hi).toBe(1);
    for (let i = 0; i < SCENES.length - 1; i++) {
      // no gap, no overlap: each hi is the next lo
      expect(SCENES[i]?.hi).toBe(SCENES[i + 1]?.lo);
    }
  });
});

describe("resolveScene", () => {
  it("resolves each scene's lo (inclusive) to that scene", () => {
    SCENES.forEach((s, i) => {
      expect(resolveScene(s.lo)).toBe(i);
    });
  });

  it("resolves an interior boundary hi (exclusive) to the NEXT scene", () => {
    for (let i = 0; i < SCENES.length - 1; i++) {
      expect(resolveScene(SCENES[i]?.hi ?? -1)).toBe(i + 1);
    }
  });

  it("lands t === 1 in the closed final scene", () => {
    expect(resolveScene(1)).toBe(8);
    expect(resolveScene(0.999)).toBe(8);
  });

  it("clamps out-of-range t", () => {
    expect(resolveScene(-0.5)).toBe(0);
    expect(resolveScene(2)).toBe(8);
  });

  it("resolves representative interior values", () => {
    expect(resolveScene(0.049)).toBe(0);
    expect(resolveScene(0.2)).toBe(1);
    expect(resolveScene(0.35)).toBe(2);
    expect(resolveScene(0.45)).toBe(3);
    expect(resolveScene(0.55)).toBe(4);
    expect(resolveScene(0.61)).toBe(5);
    expect(resolveScene(0.7)).toBe(6);
    expect(resolveScene(0.9)).toBe(7);
  });
});

describe("sceneProgress", () => {
  it("is 0 at lo, 0.5 at midpoint, clamps to [0,1]", () => {
    const canyon = 1;
    const s = SCENES[canyon];
    expect(s).toBeDefined();
    if (!s) return;
    expect(sceneProgress(s.lo, canyon)).toBe(0);
    expect(sceneProgress((s.lo + s.hi) / 2, canyon)).toBeCloseTo(0.5, 6);
    expect(sceneProgress(s.hi, canyon)).toBe(1);
    expect(sceneProgress(s.lo - 1, canyon)).toBe(0);
    expect(sceneProgress(s.hi + 1, canyon)).toBe(1);
  });

  it("returns 0 for an out-of-range scene index", () => {
    expect(sceneProgress(0.5, 99)).toBe(0);
  });
});

describe("sceneStartT / sceneById", () => {
  it("sceneStartT returns the scene lo", () => {
    expect(sceneStartT(3)).toBe(SCENES[3]?.lo);
    expect(sceneStartT(99)).toBe(0);
  });

  it("sceneById resolves a hash id to its scene", () => {
    expect(sceneById("the-deep")?.index).toBe(3);
    expect(sceneById("finale")?.index).toBe(8);
    expect(sceneById("nope")).toBeUndefined();
  });
});
