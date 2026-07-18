import { describe, expect, it } from "vitest";

import {
  frozenTrackHeightPx,
  playheadToScrollY,
  scrollableRangePx,
  scrollYToPlayhead,
} from "./scroll-track";

// SCROLL_TRACK_SVH is 3000 svh == 30 viewport heights.

describe("frozenTrackHeightPx", () => {
  it("freezes 3000 svh to 30x the viewport height", () => {
    expect(frozenTrackHeightPx(800)).toBe(24000);
    expect(frozenTrackHeightPx(900)).toBe(27000);
  });

  it("rounds to an integer px", () => {
    expect(Number.isInteger(frozenTrackHeightPx(811))).toBe(true);
  });

  it("is 0 for a zero viewport height", () => {
    expect(frozenTrackHeightPx(0)).toBe(0);
  });
});

describe("scrollableRangePx", () => {
  it("is the track height minus one viewport", () => {
    expect(scrollableRangePx(800)).toBe(23200);
  });

  it("is never negative", () => {
    expect(scrollableRangePx(0)).toBe(0);
  });
});

describe("scrollYToPlayhead", () => {
  it("maps 0..range to 0..1", () => {
    expect(scrollYToPlayhead(0, 800)).toBe(0);
    expect(scrollYToPlayhead(23200, 800)).toBe(1);
    expect(scrollYToPlayhead(11600, 800)).toBeCloseTo(0.5, 6);
  });

  it("clamps beyond the ends", () => {
    expect(scrollYToPlayhead(-100, 800)).toBe(0);
    expect(scrollYToPlayhead(1e9, 800)).toBe(1);
  });

  it("degenerates safely at a zero viewport", () => {
    expect(scrollYToPlayhead(500, 0)).toBe(0);
  });

  it("resolves a NaN scrollY to 0 instead of NaN", () => {
    expect(scrollYToPlayhead(NaN, 800)).toBe(0);
  });
});

describe("playheadToScrollY <-> scrollYToPlayhead", () => {
  it("inverts scrollYToPlayhead", () => {
    expect(playheadToScrollY(0, 800)).toBe(0);
    expect(playheadToScrollY(1, 800)).toBe(23200);
    expect(playheadToScrollY(0.5, 800)).toBe(11600);
  });

  it("round-trips within epsilon", () => {
    const t = 0.4237;
    const y = playheadToScrollY(t, 800);
    expect(scrollYToPlayhead(y, 800)).toBeCloseTo(t, 6);
  });

  it("clamps a below-range playhead to scrollY 0", () => {
    expect(playheadToScrollY(-5, 800)).toBe(0);
  });

  it("clamps an above-range playhead to the full scrollable range", () => {
    expect(playheadToScrollY(5, 800)).toBe(scrollableRangePx(800));
  });

  it("resolves a NaN playhead to scrollY 0 instead of NaN", () => {
    expect(playheadToScrollY(NaN, 800)).toBe(0);
  });
});
