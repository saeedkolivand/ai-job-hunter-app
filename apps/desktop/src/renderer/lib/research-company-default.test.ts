import { describe, expect, it } from 'vitest';

import { shouldSeedResearchDefault } from './research-company-default';

describe('shouldSeedResearchDefault', () => {
  it('does not seed until the capability resolves', () => {
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: false,
        supportsWebSearch: true,
        userTouched: false,
        lastSeededValue: null,
      })
    ).toEqual({ seed: false, value: true });
  });

  it('seeds the resolved capability on the first resolve (never seeded yet)', () => {
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: true,
        userTouched: false,
        lastSeededValue: null,
      })
    ).toEqual({ seed: true, value: true });

    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: false,
        userTouched: false,
        lastSeededValue: null,
      })
    ).toEqual({ seed: true, value: false });
  });

  it('does not re-seed a value already applied', () => {
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: true,
        userTouched: false,
        lastSeededValue: true,
      }).seed
    ).toBe(false);
  });

  it('re-seeds when a mid-session model switch flips the capability (still untouched)', () => {
    // Was seeded ON for a searcher, model switched to a non-searcher.
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: false,
        userTouched: false,
        lastSeededValue: true,
      })
    ).toEqual({ seed: true, value: false });

    // Was seeded OFF, model switched to a web-search-capable one.
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: true,
        userTouched: false,
        lastSeededValue: false,
      })
    ).toEqual({ seed: true, value: true });
  });

  it('never seeds once the user has toggled it — the explicit choice is sticky', () => {
    // Capability flips ON but the user turned it OFF: no clobber.
    expect(
      shouldSeedResearchDefault({
        capabilityResolved: true,
        supportsWebSearch: true,
        userTouched: true,
        lastSeededValue: false,
      }).seed
    ).toBe(false);
  });
});
