import { describe, expect, it } from 'vitest';

import { lighten, lightenHex, luminance, parseHex, readableForeground, toHex } from './color';

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
