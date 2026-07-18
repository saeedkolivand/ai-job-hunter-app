import { describe, expect, it } from "vitest";

import { depthMeters, formatDepth, formatTimecode, timecodeSeconds } from "./timecode";

describe("formatTimecode", () => {
  it("counts 00:00 -> 02:40 across the playhead", () => {
    expect(formatTimecode(0)).toBe("00:00");
    expect(formatTimecode(1)).toBe("02:40");
    expect(formatTimecode(0.5)).toBe("01:20");
    expect(formatTimecode(0.25)).toBe("00:40");
  });

  it("zero-pads and clamps out-of-range t", () => {
    expect(formatTimecode(-1)).toBe("00:00");
    expect(formatTimecode(2)).toBe("02:40");
    // 6 seconds in -> 00:06 (single-digit seconds padded)
    expect(formatTimecode(6 / 160)).toBe("00:06");
  });

  it("never renders NaN for a non-finite playhead", () => {
    expect(formatTimecode(NaN)).toBe("00:00");
    expect(formatTimecode(NaN)).not.toContain("NaN");
    expect(timecodeSeconds(NaN)).toBe(0);
  });
});

describe("depthMeters", () => {
  it("is 0 above the surface and after the resurface", () => {
    expect(depthMeters(0)).toBe(0);
    expect(depthMeters(0.2)).toBe(0); // still falling through air
    expect(depthMeters(0.3)).toBe(0); // exactly the surface
    expect(depthMeters(0.88)).toBe(0); // exactly the resurface
    expect(depthMeters(0.95)).toBe(0);
    expect(depthMeters(1)).toBe(0);
  });

  it("peaks at the bottom (the catch)", () => {
    expect(depthMeters(0.61)).toBe(1120);
  });

  it("increases monotonically on the descent", () => {
    expect(depthMeters(0.4)).toBeLessThan(depthMeters(0.5));
    expect(depthMeters(0.5)).toBeLessThan(depthMeters(0.6));
    expect(depthMeters(0.6)).toBeLessThan(depthMeters(0.61));
  });

  it("decreases monotonically on the ascent", () => {
    expect(depthMeters(0.65)).toBeGreaterThan(depthMeters(0.75));
    expect(depthMeters(0.75)).toBeGreaterThan(depthMeters(0.85));
  });
});

describe("formatDepth", () => {
  it("renders whole meters with an m suffix", () => {
    expect(formatDepth(0)).toBe("0 m");
    expect(formatDepth(0.61)).toBe("1120 m");
  });

  it("never renders NaN for a non-finite playhead", () => {
    expect(depthMeters(NaN)).toBe(0);
    expect(formatDepth(NaN)).toBe("0 m");
    expect(formatDepth(NaN)).not.toContain("NaN");
  });
});
