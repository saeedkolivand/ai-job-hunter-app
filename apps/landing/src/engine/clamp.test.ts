import { describe, expect, it } from "vitest";

import { clamp01 } from "./clamp";

describe("clamp01", () => {
  it("passes through in-range values unchanged", () => {
    expect(clamp01(0)).toBe(0);
    expect(clamp01(1)).toBe(1);
    expect(clamp01(0.42)).toBe(0.42);
  });

  it("clamps out-of-range finite values", () => {
    expect(clamp01(-5)).toBe(0);
    expect(clamp01(5)).toBe(1);
  });

  it("resolves non-finite input to 0 instead of leaking through", () => {
    expect(clamp01(NaN)).toBe(0);
    expect(clamp01(Infinity)).toBe(0);
    expect(clamp01(-Infinity)).toBe(0);
  });
});
