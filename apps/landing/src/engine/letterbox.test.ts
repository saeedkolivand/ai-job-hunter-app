import { describe, expect, it } from "vitest";

import { letterboxFlex } from "./letterbox";

describe("letterboxFlex (splash-beat chrome hook, pure f(t))", () => {
  it("is zero everywhere outside the splash beat", () => {
    for (const t of [0, 0.1, 0.29, 0.4, 0.5, 0.8, 1]) {
      expect(letterboxFlex(t)).toBe(0);
    }
  });

  it("widens at the surface impact and eases back out", () => {
    expect(letterboxFlex(0.32)).toBeGreaterThan(0.5); // near the peak
    expect(letterboxFlex(0.32)).toBeGreaterThan(letterboxFlex(0.3)); // rising into impact
    expect(letterboxFlex(0.32)).toBeGreaterThan(letterboxFlex(0.37)); // relaxing after
  });

  it("stays within [0, 1]", () => {
    for (let i = 0; i <= 100; i++) {
      const v = letterboxFlex(i / 100);
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(1);
    }
  });

  it("is deterministic (scrub-safe both directions)", () => {
    expect(letterboxFlex(0.315)).toBe(letterboxFlex(0.315));
  });
});
