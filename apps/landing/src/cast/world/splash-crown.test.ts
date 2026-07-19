import { describe, expect, it } from "vitest";

import { CROWN_FRAMES, crownVertexAt, normalizeAngle } from "./splash-crown";

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
});

describe("normalizeAngle", () => {
  it("wraps into [0, 2PI)", () => {
    expect(normalizeAngle(-Math.PI)).toBeCloseTo(Math.PI, 10);
    expect(normalizeAngle(0)).toBe(0);
    expect(normalizeAngle(3 * Math.PI)).toBeCloseTo(Math.PI, 10);
  });
});
