import { describe, expect, it } from "vitest";

import {
  cameraLookUpY,
  cameraSwayX,
  cameraY,
  canyonActive,
  canyonFogRGB,
  type DeskPropState,
  hash01,
  stormActiveCount,
  stormDensity,
  stormInstance,
  towerInstance,
  WORLD_HEIGHT,
  writeDeskProp,
} from "./canyon-layout";

describe("hash01", () => {
  it("is deterministic for a given input", () => {
    expect(hash01(5)).toBe(hash01(5));
    expect(hash01(12345)).toBe(hash01(12345));
  });

  it("stays in [0, 1) and varies across inputs", () => {
    const seen = new Set<number>();
    for (let i = 0; i < 500; i++) {
      const v = hash01(i);
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
      seen.add(v);
    }
    // no catastrophic collisions across 500 distinct inputs
    expect(seen.size).toBeGreaterThan(490);
  });
});

describe("stormDensity", () => {
  it("is 0 before the canyon and after the surface, in [0,1] between", () => {
    expect(stormDensity(0)).toBe(0);
    expect(stormDensity(0.04)).toBe(0);
    expect(stormDensity(0.5)).toBe(0); // faded out past the surface
    for (let t = 0; t <= 1; t += 0.02) {
      const d = stormDensity(t);
      expect(d).toBeGreaterThanOrEqual(0);
      expect(d).toBeLessThanOrEqual(1);
    }
  });

  it("thickens (rises) across the canyon to full near the floor", () => {
    expect(stormDensity(0.1)).toBeLessThan(stormDensity(0.2));
    expect(stormDensity(0.2)).toBeLessThan(stormDensity(0.26));
    expect(stormDensity(0.28)).toBeCloseTo(1, 5);
  });
});

describe("stormActiveCount", () => {
  it("reveals none at t=0 and never exceeds the max", () => {
    expect(stormActiveCount(0, 4000)).toBe(0);
    for (let t = 0; t <= 1; t += 0.05) {
      const n = stormActiveCount(t, 4000);
      expect(Number.isInteger(n)).toBe(true);
      expect(n).toBeGreaterThanOrEqual(0);
      expect(n).toBeLessThanOrEqual(4000);
    }
  });

  it("equals round(density * max)", () => {
    expect(stormActiveCount(0.2, 2000)).toBe(Math.round(stormDensity(0.2) * 2000));
  });
});

describe("camera path", () => {
  it("cameraY is the shared straight-down descent, monotonic", () => {
    expect(cameraY(0)).toBeCloseTo(0, 10); // +0/-0 tolerant
    expect(cameraY(1)).toBe(-WORLD_HEIGHT);
    expect(cameraY(0.5)).toBe(-WORLD_HEIGHT / 2);
    expect(cameraY(0.3)).toBeLessThan(cameraY(0.1));
  });

  it("canyon framing (sway + look-up) is zero outside the canyon, active inside", () => {
    expect(canyonActive(0)).toBe(0);
    expect(canyonActive(0.6)).toBe(0);
    expect(cameraSwayX(0)).toBe(0);
    expect(cameraLookUpY(0)).toBe(0);
    expect(cameraLookUpY(0.6)).toBe(0);
    // mid-canyon the backward-fall look-up is engaged
    expect(canyonActive(0.17)).toBeCloseTo(1, 5);
    expect(cameraLookUpY(0.17)).toBeGreaterThan(0);
  });

  it("is a pure function of t (same t -> same values)", () => {
    expect(cameraSwayX(0.2)).toBe(cameraSwayX(0.2));
    expect(cameraLookUpY(0.2)).toBe(cameraLookUpY(0.2));
  });
});

describe("canyonFogRGB", () => {
  it("writes 3 channels in [0,1] without allocating a new array", () => {
    const out: [number, number, number] = [0, 0, 0];
    const same = out;
    canyonFogRGB(0.2, out);
    expect(same).toBe(out);
    for (const c of out) {
      expect(c).toBeGreaterThanOrEqual(0);
      expect(c).toBeLessThanOrEqual(1);
    }
  });

  it("grades sodium-orange (warm) up top to cold blue in the deep", () => {
    const top: [number, number, number] = [0, 0, 0];
    const deep: [number, number, number] = [0, 0, 0];
    canyonFogRGB(0.05, top);
    canyonFogRGB(0.3, deep);
    expect(top[0]).toBeGreaterThan(top[2]); // warm: red > blue
    expect(deep[2]).toBeGreaterThan(deep[0]); // cold: blue > red
  });
});

describe("stormInstance", () => {
  it("is deterministic and within the canyon band", () => {
    for (const i of [0, 1, 7, 100, 3999]) {
      const a = stormInstance(i);
      const b = stormInstance(i);
      expect(a).toEqual(b);
      expect(a.x).toBeGreaterThanOrEqual(-5);
      expect(a.x).toBeLessThanOrEqual(5);
      expect(a.y).toBeLessThanOrEqual(-8);
      expect(a.y).toBeGreaterThanOrEqual(-82);
      expect(a.scale).toBeGreaterThanOrEqual(0.5);
      expect(a.scale).toBeLessThanOrEqual(1.2);
      expect(a.seed).toBeGreaterThanOrEqual(0);
      expect(a.seed).toBeLessThan(1);
    }
  });

  it("scatters neighbours to different positions", () => {
    expect(stormInstance(0).x).not.toBe(stormInstance(1).x);
    expect(stormInstance(0).y).not.toBe(stormInstance(1).y);
  });
});

describe("towerInstance", () => {
  it("is deterministic and splits into two walls by parity", () => {
    const left = towerInstance(0);
    const right = towerInstance(1);
    expect(towerInstance(0)).toEqual(left);
    expect(left.x).toBeLessThan(0); // even index -> left wall
    expect(right.x).toBeGreaterThan(0); // odd index -> right wall
  });

  it("keeps dimensions positive and within the layout envelope", () => {
    for (const i of [0, 1, 5, 55, 159]) {
      const t = towerInstance(i);
      expect(t.w).toBeGreaterThan(0);
      expect(t.h).toBeGreaterThan(0);
      expect(t.d).toBeGreaterThan(0);
      expect(Math.abs(t.x)).toBeGreaterThanOrEqual(7);
      expect(t.seed).toBeGreaterThanOrEqual(0);
      expect(t.seed).toBeLessThan(1);
    }
  });
});

describe("writeDeskProp", () => {
  const scratch: DeskPropState = { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, visible: false };

  it("is invisible outside the canyon, visible inside", () => {
    writeDeskProp(0, 0, scratch);
    expect(scratch.visible).toBe(false);
    writeDeskProp(0, 0.6, scratch);
    expect(scratch.visible).toBe(false);
    writeDeskProp(0, 0.17, scratch);
    expect(scratch.visible).toBe(true);
  });

  it("is deterministic and tracks the camera descent", () => {
    const a: DeskPropState = { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, visible: false };
    const b: DeskPropState = { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, visible: false };
    writeDeskProp(1, 0.17, a);
    writeDeskProp(1, 0.17, b);
    expect(a).toEqual(b);
    // prop y falls with the camera (descends as t grows)
    writeDeskProp(1, 0.1, a);
    writeDeskProp(1, 0.25, b);
    expect(b.y).toBeLessThan(a.y);
  });

  it("ignores an out-of-range prop index", () => {
    writeDeskProp(99, 0.17, scratch);
    expect(scratch.visible).toBe(false);
  });
});
