import { describe, expect, it } from "vitest";

import { nextMode } from "./motion-machine";

describe("nextMode", () => {
  it("resolves the initial gate from pending", () => {
    expect(nextMode("pending", { type: "gate-resolved", pass: true })).toBe("gl-live");
    expect(nextMode("pending", { type: "gate-resolved", pass: false })).toBe("fallback");
  });

  it("reduces a live film to a slideshow, and is a no-op otherwise", () => {
    expect(nextMode("gl-live", { type: "reduce-motion" })).toBe("slideshow");
    expect(nextMode("fallback", { type: "reduce-motion" })).toBe("fallback");
    expect(nextMode("slideshow", { type: "reduce-motion" })).toBe("slideshow");
  });

  it("restores a slideshow to gl-live only when the gate still passes", () => {
    expect(nextMode("slideshow", { type: "restore-motion", gatePass: true })).toBe("gl-live");
    expect(nextMode("slideshow", { type: "restore-motion", gatePass: false })).toBe("slideshow");
  });

  it("ignores restore-motion when not on the slideshow", () => {
    expect(nextMode("gl-live", { type: "restore-motion", gatePass: true })).toBe("gl-live");
    expect(nextMode("fallback", { type: "restore-motion", gatePass: true })).toBe("fallback");
  });
});
