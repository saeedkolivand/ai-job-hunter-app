import { describe, expect, it } from "vitest";

import { cameraY } from "./canyon-layout";
import {
  cameraLookDownOffset,
  gerstnerHeight,
  gerstnerSurface,
  godrayStrength,
  sceneLuminance,
  SURFACE_WORLD_Y,
  type SurfacePoint,
  worldFog,
  type WorldLayers,
  worldLayers,
  writeWorldLayers,
} from "./water-layout";

function pt(): SurfacePoint {
  return { x: 0, y: 0, z: 0, nx: 0, ny: 1, nz: 0 };
}

describe("Gerstner surface (pure f(t, xz), scrub-safe)", () => {
  it("is deterministic: the same (x, z, t) always yields the same point", () => {
    const a = pt();
    const b = pt();
    gerstnerSurface(12.5, -7.25, 0.42, a);
    gerstnerSurface(12.5, -7.25, 0.42, b);
    expect(a).toEqual(b);
  });

  it("is reversible: re-evaluating at an earlier t retraces the exact point", () => {
    const t0 = pt();
    const mid = pt();
    const back = pt();
    gerstnerSurface(3, 4, 0.33, t0);
    gerstnerSurface(3, 4, 0.51, mid); // move forward
    gerstnerSurface(3, 4, 0.33, back); // scrub back
    expect(back).toEqual(t0);
    // and the forward frame genuinely differed (the surface actually animates)
    expect(mid.y).not.toBeCloseTo(t0.y, 6);
  });

  it("returns a unit-length normal", () => {
    const p = pt();
    for (const t of [0, 0.3, 0.44, 0.77, 1]) {
      gerstnerSurface(-18, 9, t, p);
      expect(Math.hypot(p.nx, p.ny, p.nz)).toBeCloseTo(1, 6);
    }
  });

  it("gerstnerHeight matches the full-surface y (cheap path is consistent)", () => {
    const p = pt();
    gerstnerSurface(21, -5, 0.4, p);
    expect(gerstnerHeight(21, -5, 0.4)).toBeCloseTo(p.y, 10);
  });

  it("produces finite output across the patch for finite inputs", () => {
    // The kernel mirrors the guard-free water vertex shader (callers pass finite
    // letter positions + the already-clamped playhead t) -- assert it stays finite
    // over the sampled range, corners included.
    const p = pt();
    for (const x of [-150, -37, 0, 88, 150]) {
      for (const t of [0.3, 0.44, 0.58]) {
        gerstnerSurface(x, -x * 0.5, t, p);
        expect(Number.isFinite(p.x + p.y + p.z)).toBe(true);
        expect(Number.isFinite(p.nx + p.ny + p.nz)).toBe(true);
      }
    }
  });
});

describe("scene-range visibility (worldLayers)", () => {
  it("shows only the canyon in the cold-open + canyon (scenes 0-1)", () => {
    for (const s of [0, 1]) {
      expect(worldLayers(s)).toEqual({
        canyon: true,
        water: false,
        splash: false,
        deep: false,
        markers: false,
      });
    }
  });

  it("shows canyon + water + splash at the surface (scene 2)", () => {
    expect(worldLayers(2)).toEqual({
      canyon: true,
      water: true,
      splash: true,
      deep: false,
      markers: false,
    });
  });

  it("shows water + deep in the deep (scene 3) and deep-only in the blackout (scene 4)", () => {
    expect(worldLayers(3)).toEqual({
      canyon: false,
      water: true,
      splash: false,
      deep: true,
      markers: false,
    });
    expect(worldLayers(4)).toEqual({
      canyon: false,
      water: false,
      splash: false,
      deep: true,
      markers: false,
    });
  });

  it("shows the placeholder markers only for the not-yet-built scenes 5-8", () => {
    for (const s of [5, 6, 7, 8]) {
      expect(worldLayers(s).markers).toBe(true);
      expect(worldLayers(s).deep).toBe(false);
      expect(worldLayers(s).canyon).toBe(false);
    }
  });
});

describe("writeWorldLayers (the zero-per-frame-allocation out-param path)", () => {
  it("mutates the SAME out object across calls instead of allocating a fresh one", () => {
    const out: WorldLayers = { canyon: false, water: false, splash: false, deep: false, markers: false };
    const identity = out;
    writeWorldLayers(0, out);
    expect(out).toBe(identity); // same reference -- no reallocation
    expect(out.canyon).toBe(true);
    writeWorldLayers(3, out);
    expect(out).toBe(identity);
    expect(out).toEqual({ canyon: false, water: true, splash: false, deep: true, markers: false });
  });

  it("agrees with worldLayers() (the fresh-object wrapper) for every scene", () => {
    const out: WorldLayers = { canyon: false, water: false, splash: false, deep: false, markers: false };
    for (let s = 0; s <= 8; s++) {
      writeWorldLayers(s, out);
      expect(out).toEqual(worldLayers(s));
    }
  });
});

describe("luminance target + godray strength (pure f(t))", () => {
  it("sceneLuminance is full through the canyon and never leaves [0, 1]", () => {
    for (const t of [0, 0.1, 0.29, 0.3]) expect(sceneLuminance(t)).toBe(1);
    for (const t of [0, 0.3, 0.4, 0.52, 0.58, 0.7, 1]) {
      const l = sceneLuminance(t);
      expect(l).toBeGreaterThanOrEqual(0);
      expect(l).toBeLessThanOrEqual(1);
    }
  });

  it("sceneLuminance dims monotonically from the surface into the blackout", () => {
    const samples = [0.3, 0.34, 0.38, 0.45, 0.52, 0.55, 0.58];
    for (let i = 0; i < samples.length - 1; i++) {
      const hi = sceneLuminance(samples[i] as number);
      const lo = sceneLuminance(samples[i + 1] as number);
      expect(lo).toBeLessThanOrEqual(hi + 1e-9);
    }
    expect(sceneLuminance(0.58)).toBeLessThan(0.1); // near-black by the blackout
  });

  it("godrayStrength is zero outside the deep band and positive within it", () => {
    expect(godrayStrength(0.3)).toBe(0); // above the surface
    expect(godrayStrength(0.57)).toBe(0); // gone by the blackout
    expect(godrayStrength(0.42)).toBeGreaterThan(0); // mid-deep
  });
});

describe("cameraLookDownOffset (the M3 review round-2 camera-aim fix)", () => {
  it("is exactly zero outside the water-crossing window", () => {
    expect(cameraLookDownOffset(0.2)).toBe(0); // deep in the canyon
    expect(cameraLookDownOffset(0.27)).toBe(0); // exactly the ramp-in edge
    expect(cameraLookDownOffset(0.58)).toBe(0); // exactly the blackout cutoff
    expect(cameraLookDownOffset(0.7)).toBe(0); // well past the blackout
  });

  it("holds the look-at target close to the fixed water plane through scene 2", () => {
    // Numerically verified (see the M3 handoff log for the full sweep + the
    // frustum-angle check this bound was chosen from): the OLD offset let the
    // aim drift ~20 units from the plane by mid scene-2, pointing the camera
    // away from the paved ocean + splash crown entirely. The fixed offset keeps
    // the aim within a small, bounded distance of the plane for the WHOLE of
    // scene 2 [0.30, 0.38), not just an instant at the crossing.
    for (let t = 0.3; t <= 0.38; t += 0.005) {
      const aimY = cameraY(t) + cameraLookDownOffset(t);
      expect(Math.abs(aimY - SURFACE_WORLD_Y)).toBeLessThan(8);
    }
  });

  it("converges to exactly the deep-scene magnitude once solidly past the crossing", () => {
    // Matches the ORIGINAL constant (-12) the coordinator already approved for
    // the deep/blackout framing at t=0.45 -- this fix's transition-window
    // change must not alter that already-approved look.
    for (const t of [0.43, 0.45, 0.48, 0.5, 0.53]) {
      expect(cameraLookDownOffset(t)).toBeCloseTo(-12, 6);
    }
  });
});

describe("worldFog (cold-open density pullback + the whole-descent grade)", () => {
  const rgb: [number, number, number] = [0, 0, 0];

  it("reduces density at the cold open relative to the established canyon density", () => {
    const openDensity = worldFog(0.02, rgb);
    const canyonDensity = worldFog(0.15, rgb); // deep in the canyon fall
    expect(openDensity).toBeLessThan(canyonDensity);
    expect(openDensity).toBeGreaterThan(0); // never fully clear (still atmosphere)
  });

  it("ramps the cold-open density up to the canyon density by t=0.05", () => {
    const at0 = worldFog(0, rgb);
    const at005 = worldFog(0.05, rgb);
    const canyonDensity = worldFog(0.15, rgb);
    expect(at0).toBeLessThan(at005);
    expect(at005).toBeCloseTo(canyonDensity, 6);
  });

  it("returns a valid, finite density and rgb across the whole descent", () => {
    for (let t = 0; t <= 1; t += 0.05) {
      const d = worldFog(t, rgb);
      expect(Number.isFinite(d)).toBe(true);
      expect(d).toBeGreaterThan(0);
      expect(Number.isFinite(rgb[0] + rgb[1] + rgb[2])).toBe(true);
    }
  });
});

describe("world anchors", () => {
  it("the surface plane sits below the camera's surface-crossing altitude", () => {
    expect(Number.isFinite(SURFACE_WORLD_Y)).toBe(true);
    expect(SURFACE_WORLD_Y).toBeLessThan(0);
  });
});
