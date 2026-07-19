import { describe, expect, it } from "vitest";

import {
  frameRowV,
  vatFrameIndex,
  type VatFrameSample,
  type VatMeta,
  vatProgress,
  writeVatFrameIndex,
} from "./vat";

describe("vatFrameIndex (deterministic frame pair + blend, pure f(progress))", () => {
  it("returns a degenerate sample for a single-frame clip", () => {
    expect(vatFrameIndex(0.5, 1)).toEqual({ a: 0, b: 0, blend: 0 });
  });

  it("lands on the first frame at progress 0 and the last frame at progress 1", () => {
    expect(vatFrameIndex(0, 64)).toEqual({ a: 0, b: 1, blend: 0 });
    expect(vatFrameIndex(1, 64)).toEqual({ a: 63, b: 63, blend: 0 });
  });

  it("hits an exact interior frame with zero blend", () => {
    // frames=5 -> pos = 0.5 * 4 = 2.0 -> frame 2, no blend
    expect(vatFrameIndex(0.5, 5)).toEqual({ a: 2, b: 3, blend: 0 });
  });

  it("interpolates between the two nearest frames (blend in [0, 1))", () => {
    const s = vatFrameIndex(0.3, 5); // pos = 1.2 -> a=1, b=2, blend=0.2
    expect(s.a).toBe(1);
    expect(s.b).toBe(2);
    expect(s.blend).toBeCloseTo(0.2, 6);
    expect(s.blend).toBeGreaterThanOrEqual(0);
    expect(s.blend).toBeLessThan(1);
  });

  it("advances monotonically: a + blend never decreases as progress rises", () => {
    let prev = -1;
    for (let i = 0; i <= 100; i++) {
      const s = vatFrameIndex(i / 100, 64);
      const abs = s.a + s.blend;
      expect(abs).toBeGreaterThanOrEqual(prev - 1e-9);
      prev = abs;
    }
  });

  it("is reversible: the same progress always yields the same sample", () => {
    expect(vatFrameIndex(0.37, 64)).toEqual(vatFrameIndex(0.37, 64));
  });

  it("clamps out-of-range and non-finite progress", () => {
    expect(vatFrameIndex(-5, 64)).toEqual({ a: 0, b: 1, blend: 0 });
    expect(vatFrameIndex(9, 64)).toEqual({ a: 63, b: 63, blend: 0 });
    const nan = vatFrameIndex(Number.NaN, 64);
    expect(nan.a).toBe(0);
    expect(Number.isFinite(nan.blend)).toBe(true);
  });
});

describe("writeVatFrameIndex (the zero-per-frame-allocation out-param path)", () => {
  it("mutates the SAME out object across calls instead of allocating a fresh one", () => {
    const out: VatFrameSample = { a: 0, b: 0, blend: 0 };
    const identity = out;
    writeVatFrameIndex(0, 64, out);
    expect(out).toBe(identity); // same reference -- no reallocation
    expect(out).toEqual({ a: 0, b: 1, blend: 0 });
    writeVatFrameIndex(0.5, 5, out);
    expect(out).toBe(identity);
    expect(out).toEqual({ a: 2, b: 3, blend: 0 });
  });

  it("agrees with vatFrameIndex() (the fresh-object wrapper) across the clip", () => {
    const out: VatFrameSample = { a: 0, b: 0, blend: 0 };
    for (let i = 0; i <= 100; i++) {
      const p = i / 100;
      writeVatFrameIndex(p, 64, out);
      expect(out).toEqual(vatFrameIndex(p, 64));
    }
  });
});

describe("frameRowV (row-centre texture V)", () => {
  it("returns the half-texel row centre, strictly inside (0, 1)", () => {
    expect(frameRowV(0, 64)).toBeCloseTo(0.5 / 64, 10);
    expect(frameRowV(63, 64)).toBeCloseTo(63.5 / 64, 10);
    expect(frameRowV(0, 64)).toBeGreaterThan(0);
    expect(frameRowV(63, 64)).toBeLessThan(1);
  });
});

describe("vatProgress (scene progress -> clip play window)", () => {
  const meta: VatMeta = { frames: 64, vertices: 100, duration: 0.85 };

  it("reaches 1 when the scene passes the clip's play window, then holds", () => {
    expect(vatProgress(0, meta)).toBe(0);
    expect(vatProgress(0.85, meta)).toBeCloseTo(1, 6);
    expect(vatProgress(1, meta)).toBe(1); // held on the last frame while the camera sinks on
  });

  it("clamps non-finite scene progress to 0", () => {
    expect(vatProgress(Number.NaN, meta)).toBe(0);
  });
});
