import { describe, expect, it } from "vitest";

import { LuminanceClamp } from "./luminance-clamp";

describe("LuminanceClamp (WCAG 2.3.1 luminance-velocity clamp)", () => {
  it("caps the slew: applied moves by at most maxSlew * dt toward the target", () => {
    const c = new LuminanceClamp(0, 2); // 2 units/sec
    c.step(0, 0.016); // consume the mount-prime (target === initial -> no visible effect)
    expect(c.step(1, 0.1)).toBeCloseTo(0.2, 10); // 2 * 0.1 = 0.2, not the full 1.0
    expect(c.step(1, 0.1)).toBeCloseTo(0.4, 10);
  });

  it("caps the slew downward symmetrically", () => {
    const c = new LuminanceClamp(1, 2);
    c.step(1, 0.016); // consume the mount-prime
    expect(c.step(0, 0.1)).toBeCloseTo(0.8, 10);
  });

  it("converges to a steady target and then holds it exactly (determinism at rest)", () => {
    const c = new LuminanceClamp(0, 2);
    c.step(0, 0.016); // consume the mount-prime
    for (let i = 0; i < 200; i++) c.step(1, 0.016);
    expect(c.value).toBe(1); // snapped exactly, no asymptotic drift
    // At rest the applied value equals the pure-f(t) target every frame.
    expect(c.step(1, 0.016)).toBe(1);
    expect(c.step(1, 0.5)).toBe(1);
  });

  it("snaps exactly when the target is within reach this frame", () => {
    const c = new LuminanceClamp(0, 2);
    c.step(0, 0.016); // consume the mount-prime
    // diff (0.2) === maxSlew*dt (2 * 0.1) -> within reach -> exact snap
    expect(c.step(0.2, 0.1)).toBe(0.2);
  });

  it("reset jumps immediately with no slew limit, and the next step() slews normally from there", () => {
    const c = new LuminanceClamp(1, 2);
    c.reset(0.05);
    expect(c.value).toBe(0.05);
    expect(c.step(1, 0.1)).toBeCloseTo(0.25, 10); // 0.05 + 2*0.1 -- properly rate-limited
  });

  it("holds on a non-positive dt or a non-finite target", () => {
    const c = new LuminanceClamp(0.5, 2);
    c.step(0.5, 0.016); // consume the mount-prime (target === initial -> no visible effect)
    expect(c.step(1, 0)).toBe(0.5);
    expect(c.step(1, -1)).toBe(0.5);
    expect(c.step(Number.NaN, 0.1)).toBe(0.5);
  });

  it("is deterministic for a given (target, dt) sequence regardless of scrub direction history", () => {
    const down = new LuminanceClamp(1, 1.6);
    const up = new LuminanceClamp(0, 1.6);
    down.step(1, 0.016); // consume each instance's own mount-prime first
    up.step(0, 0.016);
    // Both driven to a steady 0.3 target long enough to converge -> identical.
    for (let i = 0; i < 500; i++) {
      down.step(0.3, 0.016);
      up.step(0.3, 0.016);
    }
    expect(down.value).toBe(0.3);
    expect(up.value).toBe(0.3);
  });
});

// Regression coverage for the M3 review HIGH: a fresh mount (or remount) landing
// at a playhead far from the constructor's throwaway `initial` -- a hash
// deep-link straight into the blackout, or the reduce->restore GL remount --
// must render the correct target on the very first frame, not visibly fade
// in/out from `initial` over the first ~maxSlew window (an inverted-WCAG flash
// at boot, the opposite of what this class exists to prevent).
describe("mount priming (a fresh mount/reset is a hard cut, not a slew)", () => {
  it("the very first step() call after construction hard-cuts to the target regardless of the slew cap", () => {
    const brightMount = new LuminanceClamp(1, 1.6); // constructed bright...
    expect(brightMount.step(0.03, 0.016)).toBe(0.03); // ...but mounts straight into a dark target
    const darkMount = new LuminanceClamp(0, 1.6); // constructed dark...
    expect(darkMount.step(1, 0.016)).toBe(1); // ...but mounts straight into a bright target
  });

  it("only the very first call is primed -- every subsequent call is rate-limited normally", () => {
    const c = new LuminanceClamp(1, 1.6);
    expect(c.step(0.03, 0.016)).toBe(0.03); // mount prime: hard cut
    expect(c.step(1, 0.016)).toBeCloseTo(0.03 + 1.6 * 0.016, 10); // now properly slewed
  });

  it("an explicit reset() before any step() call also counts as priming (no double hard-cut)", () => {
    const c = new LuminanceClamp(1, 1.6);
    c.reset(0.5); // an external hard cut before the mount's first frame ever ticks
    expect(c.step(1, 0.016)).toBeCloseTo(0.5 + 1.6 * 0.016, 10); // slewed, not re-primed
  });
});
