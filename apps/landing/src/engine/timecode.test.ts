import { describe, expect, it } from "vitest";

import {
  altitudeMeters,
  depthMeters,
  formatDepth,
  formatGauge,
  formatTimecode,
  gaugeMeters,
  isAltitudePhase,
  timecodeSeconds,
} from "./timecode";

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

describe("altitudeMeters", () => {
  it("is at its max at t=0 and reaches 0 exactly at the surface", () => {
    expect(altitudeMeters(0)).toBeGreaterThan(0);
    expect(altitudeMeters(0.3)).toBe(0); // SURFACE_T
    expect(altitudeMeters(0.5)).toBe(0); // stays 0 underwater
    expect(altitudeMeters(1)).toBe(0);
  });

  it("decreases monotonically through the cold-open + canyon (never frozen)", () => {
    // regression for the M2 smoke-check bug: was frozen at depth=0 through the
    // whole fall (t=0.15, mid-canyon) instead of visibly counting down.
    expect(altitudeMeters(0)).toBeGreaterThan(altitudeMeters(0.02));
    expect(altitudeMeters(0.02)).toBeGreaterThan(altitudeMeters(0.15));
    expect(altitudeMeters(0.15)).toBeGreaterThan(altitudeMeters(0.29));
    expect(altitudeMeters(0.15)).toBeGreaterThan(0); // the specific reported t
  });
});

describe("isAltitudePhase / gaugeMeters / gaugeLabel / formatGauge", () => {
  it("is altitude phase strictly before the surface, depth phase at/after", () => {
    expect(isAltitudePhase(0)).toBe(true);
    expect(isAltitudePhase(0.15)).toBe(true);
    expect(isAltitudePhase(0.299)).toBe(true);
    expect(isAltitudePhase(0.3)).toBe(false);
    expect(isAltitudePhase(0.6)).toBe(false);
  });

  it("gaugeMeters is never 0 during the fall (the reported regression)", () => {
    expect(gaugeMeters(0.15)).toBeGreaterThan(0);
    expect(gaugeMeters(0.02)).toBeGreaterThan(0);
  });

  it("gaugeMeters matches altitude before the surface, depth after", () => {
    expect(gaugeMeters(0.1)).toBe(altitudeMeters(0.1));
    expect(gaugeMeters(0.5)).toBe(depthMeters(0.5));
  });

  it("formatGauge switches label at the surface and never renders NaN", () => {
    expect(formatGauge(0)).toMatch(/^ALT \d+ m$/);
    expect(formatGauge(0.15)).toMatch(/^ALT \d+ m$/);
    expect(formatGauge(0.5)).toMatch(/^DEPTH \d+ m$/);
    expect(formatGauge(NaN)).not.toContain("NaN");
  });

  it("moves visibly across every scene boundary (ADR-0016 'depth IS progress')", () => {
    // SCENES lo values from scene-resolver.ts, sampled without importing it to
    // keep this test dependency-free -- one reading per scene start.
    const sceneStarts = [0, 0.05, 0.3, 0.38, 0.52, 0.58, 0.64, 0.85, 0.95];
    const readings = sceneStarts.map((t) => gaugeMeters(t));
    // every consecutive pair differs -- the gauge is never frozen across a
    // whole scene transition the way the reported bug was.
    for (let i = 0; i < readings.length - 1; i++) {
      expect(readings[i]).not.toBe(readings[i + 1]);
    }
  });
});
