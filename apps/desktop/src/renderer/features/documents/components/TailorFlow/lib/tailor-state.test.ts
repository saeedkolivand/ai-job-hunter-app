import { describe, expect, it } from 'vitest';

import { buildTailorDefaults } from './tailor-state';

describe('buildTailorDefaults', () => {
  it('returns outputType "both" regardless of resume presence', () => {
    expect(buildTailorDefaults('some text').outputType).toBe('both');
    expect(buildTailorDefaults().outputType).toBe('both');
    expect(buildTailorDefaults(undefined).outputType).toBe('both');
  });

  it('returns researchCompany false by default', () => {
    expect(buildTailorDefaults('text').researchCompany).toBe(false);
    expect(buildTailorDefaults().researchCompany).toBe(false);
  });

  it('seeds resume from resumeText when provided', () => {
    expect(buildTailorDefaults('My resume text').resume).toBe('My resume text');
  });

  it('seeds resume as empty string when resumeText is undefined', () => {
    expect(buildTailorDefaults().resume).toBe('');
    expect(buildTailorDefaults(undefined).resume).toBe('');
  });

  it('seeds resume as empty string when resumeText is empty string', () => {
    expect(buildTailorDefaults('').resume).toBe('');
  });
});
