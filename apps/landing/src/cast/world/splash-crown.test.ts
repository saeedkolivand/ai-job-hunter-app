import { describe, expect, it } from "vitest";

import { CROWN_FRAMES, CROWN_RADIUS, crownVertexAt, normalizeAngle } from "./splash-crown";

function vert(): [number, number, number] {
  return [0, 0, 0];
}

describe("crownVertexAt (procedural VAT stand-in bake, deterministic)", () => {
  it("is deterministic: the same (radius, angle, frame) always bakes the same position", () => {
    const a = vert();
    const b = vert();
    crownVertexAt(3.2, 1.1, 20, CROWN_FRAMES, a);
    crownVertexAt(3.2, 1.1, 20, CROWN_FRAMES, b);
    expect(a).toEqual(b);
  });

  it("is flat (zero height) at the first and last frame -- the crown erupts then falls back", () => {
    const a = vert();
    const b = vert();
    crownVertexAt(4, 0.7, 0, CROWN_FRAMES, a);
    crownVertexAt(4, 0.7, CROWN_FRAMES - 1, CROWN_FRAMES, b);
    expect(a[1]).toBeCloseTo(0, 10); // sin(0) == 0 exactly
    expect(b[1]).toBeCloseTo(0, 10); // sin(PI) is ~1e-16, not literally 0
  });

  it("rises to a positive crown height mid-clip", () => {
    const mid = vert();
    crownVertexAt(7, 0, Math.floor(CROWN_FRAMES / 2), CROWN_FRAMES, mid);
    expect(mid[1]).toBeGreaterThan(0);
  });

  it("produces finite positions across the whole clip", () => {
    const out = vert();
    for (let f = 0; f < CROWN_FRAMES; f++) {
      crownVertexAt(5, 2.4, f, CROWN_FRAMES, out);
      expect(Number.isFinite(out[0] + out[1] + out[2])).toBe(true);
    }
  });

  it("produces finite positions across a sweep of frame, radius, AND angle", () => {
    // Broader than the single-radius/angle sweep above (PR #722 CodeRabbit ask)
    // -- covers the rest position (radius 0), the outer rim (CROWN_RADIUS), and
    // wrap-around angles, at every frame in the clip.
    const out = vert();
    const radii = [0, 1.5, CROWN_RADIUS * 0.5, CROWN_RADIUS, CROWN_RADIUS * 1.1];
    const angles = [0, Math.PI * 0.5, Math.PI, Math.PI * 1.5, Math.PI * 2 - 0.001];
    for (let f = 0; f < CROWN_FRAMES; f++) {
      for (const radius of radii) {
        for (const angle of angles) {
          crownVertexAt(radius, angle, f, CROWN_FRAMES, out);
          expect(Number.isFinite(out[0])).toBe(true);
          expect(Number.isFinite(out[1])).toBe(true);
          expect(Number.isFinite(out[2])).toBe(true);
        }
      }
    }
  });

  it("degenerate bake (frames <= 1): flat (zero height) and finite, never divides by zero", () => {
    // frames<=1 has no frame RANGE to interpolate a position within -- the
    // `frames <= 1 ? 0 : ...` guard in the fp computation exists exactly to
    // avoid a frames-1 === 0 division; assert that guard actually holds for
    // both frames=1 and the frames=0 edge (an even-more-degenerate bake).
    for (const frames of [1, 0]) {
      const out = vert();
      crownVertexAt(4, 1.2, 0, frames, out);
      expect(out[1]).toBe(0); // flat: fp is forced to 0 -> env = sin(0) = 0 -> height = 0 exactly
      expect(Number.isFinite(out[0])).toBe(true);
      expect(Number.isFinite(out[1])).toBe(true);
      expect(Number.isFinite(out[2])).toBe(true);
      // Non-zero `frame` input is also ignored/safe under the degenerate guard.
      const out2 = vert();
      crownVertexAt(4, 1.2, 30, frames, out2);
      expect(out2).toEqual(out);
    }
  });
});

describe("normalizeAngle", () => {
  it("wraps into [0, 2PI)", () => {
    expect(normalizeAngle(-Math.PI)).toBeCloseTo(Math.PI, 10);
    expect(normalizeAngle(0)).toBe(0);
    expect(normalizeAngle(3 * Math.PI)).toBeCloseTo(Math.PI, 10);
  });

  it("wraps the exact upper bound (2PI) down to 0, not up to 2PI", () => {
    // a % a === 0 exactly in IEEE 754 (no rounding error), so this must be an
    // EXACT 0, not merely close to it -- the untested boundary the interval is
    // half-open [0, 2PI) around.
    expect(normalizeAngle(Math.PI * 2)).toBe(0);
  });
});
