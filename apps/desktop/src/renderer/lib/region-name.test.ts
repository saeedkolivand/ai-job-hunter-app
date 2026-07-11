import { describe, expect, it } from 'vitest';

import { regionName } from './region-name';

describe('regionName', () => {
  it('resolves a lowercase ISO alpha-2 code to a localized country name', () => {
    expect(regionName('de', 'en')).toBe('Germany');
    expect(regionName('us', 'en')).toBe('United States');
  });

  it('localizes the name by locale', () => {
    expect(regionName('de', 'de')).toBe('Deutschland');
  });

  it('accepts an already-uppercase code', () => {
    expect(regionName('GB', 'en')).toBe('United Kingdom');
  });

  it('falls back to the uppercased code for a structurally-invalid input', () => {
    // length !== 2 short-circuits before DisplayNames (which would throw).
    expect(regionName('x', 'en')).toBe('X');
    expect(regionName('', 'en')).toBe('');
  });

  it('falls back to the uppercased code when DisplayNames rejects a length-2 non-region', () => {
    // 'A1' is length-2 yet not a valid region subtag → DisplayNames throws → caught.
    expect(regionName('a1', 'en')).toBe('A1');
  });
});
