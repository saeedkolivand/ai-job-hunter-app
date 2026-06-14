import { describe, expect, it } from 'vitest';

import {
  lighten,
  lightenHex,
  luminance,
  parseHex,
  readableForeground,
  rotateHueHex,
  toHex,
} from './color';

describe('parseHex', () => {
  it('parses #rrggbb', () => {
    expect(parseHex('#a855f7')).toEqual({ r: 168, g: 85, b: 247 });
  });
  it('parses without a leading #', () => {
    expect(parseHex('a855f7')).toEqual({ r: 168, g: 85, b: 247 });
  });
  it('expands #rgb shorthand', () => {
    expect(parseHex('#abc')).toEqual({ r: 0xaa, g: 0xbb, b: 0xcc });
  });
  it('returns null for invalid input', () => {
    expect(parseHex('not-a-color')).toBeNull();
    expect(parseHex('#12')).toBeNull();
    expect(parseHex('#xyzxyz')).toBeNull();
  });
});

describe('toHex', () => {
  it('round-trips and clamps out-of-range channels', () => {
    expect(toHex({ r: 168, g: 85, b: 247 })).toBe('#a855f7');
    expect(toHex({ r: -10, g: 300, b: 0 })).toBe('#00ff00');
  });
});

describe('luminance', () => {
  it('is 0 for black and 1 for white', () => {
    expect(luminance({ r: 0, g: 0, b: 0 })).toBeCloseTo(0, 5);
    expect(luminance({ r: 255, g: 255, b: 255 })).toBeCloseTo(1, 5);
  });
  it('ranks a pale color brighter than a dark one', () => {
    expect(luminance({ r: 255, g: 230, b: 128 })).toBeGreaterThan(
      luminance({ r: 168, g: 85, b: 247 })
    );
  });
});

describe('lighten / lightenHex', () => {
  it('mixes toward white by the given amount', () => {
    expect(lighten({ r: 0, g: 0, b: 0 }, 0.5)).toEqual({ r: 127.5, g: 127.5, b: 127.5 });
    expect(lighten({ r: 100, g: 100, b: 100 }, 0)).toEqual({ r: 100, g: 100, b: 100 });
  });
  it('clamps the amount to [0,1]', () => {
    expect(lighten({ r: 0, g: 0, b: 0 }, 2)).toEqual({ r: 255, g: 255, b: 255 });
  });
  it('lightenHex returns a lighter hex, or null on invalid input', () => {
    expect(lightenHex('#000000', 0.5)).toBe('#808080');
    expect(lightenHex('xyz', 0.3)).toBeNull();
  });
});

describe('readableForeground', () => {
  it('returns a near-white label on a dark accent', () => {
    expect(readableForeground('#a855f7')).toBe('#ffffff');
    expect(readableForeground('#000000')).toBe('#ffffff');
  });
  it('returns a dark label on a pale/bright accent', () => {
    expect(readableForeground('#ffe680')).toBe('#1d1d1f');
    expect(readableForeground('#ffffff')).toBe('#1d1d1f');
  });
  it('returns null on invalid input', () => {
    expect(readableForeground('nope')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// rotateHueHex
// ---------------------------------------------------------------------------

/** Derive hue in degrees [0, 360) from a #rrggbb string. Returns -1 when invalid. */
function hueFromHex(hex: string): number {
  const rgb = parseHex(hex);
  if (!rgb) return -1;
  const r = rgb.r / 255;
  const g = rgb.g / 255;
  const b = rgb.b / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return 0; // achromatic
  let h: number;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return (h * 60 + 360) % 360;
}

describe('rotateHueHex', () => {
  it('violet #a855f7 rotated -30° lands in the indigo/blue family (~240°)', () => {
    const result = rotateHueHex('#a855f7', -30);
    expect(result).not.toBeNull();
    // Result must be a valid #rrggbb and different from the input.
    expect(result).toMatch(/^#[0-9a-f]{6}$/);
    expect(result).not.toBe('#a855f7');
    // Hue must be near 240° (±6° tolerance to absorb rounding).
    const hue = hueFromHex(result ?? '');
    expect(Math.abs(hue - 240)).toBeLessThanOrEqual(6);
  });

  it('a 0° rotation returns the same color within rounding', () => {
    const input = '#a855f7';
    const result = rotateHueHex(input, 0);
    expect(result).not.toBeNull();
    // Round-trip: parse both and compare channels within ±1 (integer rounding).
    const orig = parseHex(input);
    if (!orig) throw new Error(`parseHex failed for known-valid input: ${input}`);
    const got = parseHex(result ?? '');
    if (!got) throw new Error(`parseHex failed for rotateHueHex result: ${result}`);
    expect(Math.abs(got.r - orig.r)).toBeLessThanOrEqual(1);
    expect(Math.abs(got.g - orig.g)).toBeLessThanOrEqual(1);
    expect(Math.abs(got.b - orig.b)).toBeLessThanOrEqual(1);
  });

  it('a 360° rotation returns the same color within rounding', () => {
    const input = '#34c759';
    const result = rotateHueHex(input, 360);
    expect(result).not.toBeNull();
    const orig = parseHex(input);
    if (!orig) throw new Error(`parseHex failed for known-valid input: ${input}`);
    const got = parseHex(result ?? '');
    if (!got) throw new Error(`parseHex failed for rotateHueHex result: ${result}`);
    expect(Math.abs(got.r - orig.r)).toBeLessThanOrEqual(1);
    expect(Math.abs(got.g - orig.g)).toBeLessThanOrEqual(1);
    expect(Math.abs(got.b - orig.b)).toBeLessThanOrEqual(1);
  });

  it('hue wrap-around: near-red #ff3b30 rotated -30° gives a valid hex shifted toward magenta', () => {
    // #ff3b30 is a red (hue ~3°). Rotating -30° wraps to ~333°, the magenta/pink
    // range where blue > red channel gap is smaller (blue grows, hue shifts left).
    const result = rotateHueHex('#ff3b30', -30);
    expect(result).not.toBeNull();
    expect(result).toMatch(/^#[0-9a-f]{6}$/);
    // After rotating toward magenta the blue channel must have grown vs the input.
    const orig = parseHex('#ff3b30');
    if (!orig) throw new Error('parseHex failed for known-valid input: #ff3b30');
    const got = parseHex(result ?? '');
    if (!got) throw new Error(`parseHex failed for rotateHueHex result: ${result}`);
    expect(got.b).toBeGreaterThan(orig.b);
    // Result must still be a valid hex — no NaN / clamped-to-black artefact.
    expect(got.r + got.g + got.b).toBeGreaterThan(0);
  });

  it('invalid inputs return null', () => {
    expect(rotateHueHex('nope', -30)).toBeNull();
    expect(rotateHueHex('#12', 10)).toBeNull();
    expect(rotateHueHex('', 0)).toBeNull();
  });

  it('achromatic input (#808080) returns a valid hex that is still achromatic', () => {
    // Saturation is 0 for pure grays, so rotating the hue is a mathematical
    // no-op: S=0 means all channels must remain equal after the round-trip.
    const result = rotateHueHex('#808080', -30);
    expect(result).not.toBeNull();
    expect(result).toMatch(/^#[0-9a-f]{6}$/);
    const got = parseHex(result ?? '');
    if (!got) throw new Error(`parseHex failed for rotateHueHex result: ${result}`);
    // All channels equal within ±1 (rounding only).
    expect(Math.abs(got.r - got.g)).toBeLessThanOrEqual(1);
    expect(Math.abs(got.g - got.b)).toBeLessThanOrEqual(1);
  });
});
