import { describe, expect, it } from 'vitest';

import {
  ApplicationTrackSchema,
  ApplicationUpdateSchema,
  AutopilotTargetSchema,
  AutopilotUpdateSchema,
  JobPreferencesSchema,
} from './index';

describe('AutopilotTargetSchema', () => {
  it('defaults pages to 2', () => {
    const parsed = AutopilotTargetSchema.parse({ boards: ['linkedin'], query: 'dev' });
    expect(parsed.pages).toBe(2);
  });

  it('rejects pages above 10', () => {
    expect(() =>
      AutopilotTargetSchema.parse({ boards: ['linkedin'], query: 'dev', pages: 11 })
    ).toThrow();
  });

  it('rejects an empty boards array', () => {
    expect(() => AutopilotTargetSchema.parse({ boards: [], query: 'dev' })).toThrow();
  });

  it('accepts more than 6 boards (catalog has grown past the old cap)', () => {
    expect(() =>
      AutopilotTargetSchema.parse({
        boards: [
          'linkedin',
          'arbeitsagentur',
          'remoteok',
          'greenhouse',
          'lever',
          'ashby',
          'remotive',
        ],
        query: 'dev',
      })
    ).not.toThrow();
  });

  it('rejects a grossly oversized boards array (sanity bound, not the real cap)', () => {
    // The real dedup+truncate defense is server-side (Rust registry cap); this
    // schema-level bound only guards against a corrupt/hostile payload.
    const tooMany = Array.from({ length: 65 }, (_, i) => `board_${i}`);
    expect(() => AutopilotTargetSchema.parse({ boards: tooMany, query: 'dev' })).toThrow();
  });
});

describe('AutopilotUpdateSchema', () => {
  it('allows a partial update with status', () => {
    expect(() => AutopilotUpdateSchema.parse({ status: 'paused' })).not.toThrow();
    expect(() => AutopilotUpdateSchema.parse({})).not.toThrow();
  });

  it('rejects an invalid status', () => {
    expect(() => AutopilotUpdateSchema.parse({ status: 'deleted' })).toThrow();
  });
});

describe('JobPreferencesSchema', () => {
  it('accepts a full preferences object', () => {
    expect(() =>
      JobPreferencesSchema.parse({
        location: 'Berlin',
        techStack: [{ name: 'React', category: 'frontend' }],
      })
    ).not.toThrow();
  });

  it('accepts an empty object (all optional)', () => {
    expect(() => JobPreferencesSchema.parse({})).not.toThrow();
  });

  it('rejects a tech stack item missing a category', () => {
    expect(() => JobPreferencesSchema.parse({ techStack: [{ name: 'React' }] })).toThrow();
  });

  it('accepts a 2-letter countryCode', () => {
    expect(() =>
      JobPreferencesSchema.parse({ location: 'Berlin', countryCode: 'de' })
    ).not.toThrow();
  });

  it('rejects a malformed countryCode', () => {
    expect(() => JobPreferencesSchema.parse({ countryCode: 'deu' })).toThrow();
    expect(() => JobPreferencesSchema.parse({ countryCode: '1a' })).toThrow();
  });

  it('accepts a salaryExpectation string', () => {
    expect(() => JobPreferencesSchema.parse({ salaryExpectation: '€75,000' })).not.toThrow();
  });

  it('accepts an object with no salaryExpectation (optional, additive)', () => {
    const parsed = JobPreferencesSchema.parse({ location: 'Berlin' });
    expect(parsed.salaryExpectation).toBeUndefined();
  });

  // `jobPreferences.set` merges over the stored row: an omitted key keeps its
  // value, an explicit `null` clears the column. The clearable fields must
  // therefore accept `null` and preserve it — coercing it to `undefined` here
  // would strip the key on the way to the backend and turn a clear into a no-op.
  it('accepts and preserves null for the clearable location/countryCode fields', () => {
    const parsed = JobPreferencesSchema.parse({ location: null, countryCode: null });
    expect(parsed.location).toBeNull();
    expect(parsed.countryCode).toBeNull();
  });

  it('still rejects a malformed countryCode when it is not null', () => {
    expect(() => JobPreferencesSchema.parse({ location: null, countryCode: 'deu' })).toThrow();
  });
});

describe('ApplicationUpdateSchema — jobDescription byte-level refine', () => {
  it('accepts a valid jobDescription well under 200 000 bytes', () => {
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: 'A short description.' })
    ).not.toThrow();
  });

  it('accepts jobDescription absent (field is optional)', () => {
    expect(() => ApplicationUpdateSchema.parse({ id: 'app1' })).not.toThrow();
  });

  it('rejects a jobDescription that exceeds 200 000 bytes', () => {
    // Each 'a' is one byte — 200 001 bytes pushes past the ceiling.
    const overLimit = 'a'.repeat(200_001);
    expect(() => ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: overLimit })).toThrow(
      /200000 bytes/
    );
  });

  it('enforces a BYTE ceiling, not a character ceiling (multi-byte UTF-8)', () => {
    // '€' encodes as 3 bytes in UTF-8. 66 667 '€' chars = 200 001 bytes but
    // only 66 667 chars — under the naive char limit but over the byte limit.
    const euroCount = 66_667;
    const overLimitByBytes = '€'.repeat(euroCount);
    // Verify the fixture actually exceeds 200 000 bytes.
    expect(new TextEncoder().encode(overLimitByBytes).length).toBeGreaterThan(200_000);
    // And that the schema rejects it.
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: overLimitByBytes })
    ).toThrow(/200000 bytes/);
  });

  it('accepts a multi-byte string that stays under 200 000 bytes', () => {
    // 66 666 '€' = 199 998 bytes — just under the ceiling.
    const justUnder = '€'.repeat(66_666);
    expect(new TextEncoder().encode(justUnder).length).toBeLessThanOrEqual(200_000);
    expect(() =>
      ApplicationUpdateSchema.parse({ id: 'app1', jobDescription: justUnder })
    ).not.toThrow();
  });
});

describe('ApplicationTrackSchema — jobDescription carried from a posting', () => {
  it('accepts a track request carrying a jobDescription', () => {
    expect(() =>
      ApplicationTrackSchema.parse({
        jobUrl: 'https://example.com/job/1',
        board: 'aggregator',
        company: 'Acme',
        title: 'Engineer',
        jobDescription: 'Build things.',
      })
    ).not.toThrow();
  });

  it('accepts jobDescription absent (field is optional)', () => {
    expect(() =>
      ApplicationTrackSchema.parse({ jobUrl: 'https://example.com/job/1' })
    ).not.toThrow();
  });

  it('rejects a jobDescription that exceeds 200 000 bytes', () => {
    const overLimit = 'a'.repeat(200_001);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: overLimit })).toThrow(
      /200000 bytes/
    );
  });

  it('enforces a BYTE ceiling, not a character ceiling (multi-byte UTF-8)', () => {
    // '€' encodes as 3 bytes in UTF-8. 66 667 '€' chars = 200 001 bytes but
    // only 66 667 chars — under the naive char limit but over the byte limit.
    const euroCount = 66_667;
    const overLimitByBytes = '€'.repeat(euroCount);
    expect(new TextEncoder().encode(overLimitByBytes).length).toBeGreaterThan(200_000);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: overLimitByBytes })).toThrow(
      /200000 bytes/
    );
  });

  it('accepts a multi-byte string that stays under 200 000 bytes', () => {
    // 66 666 '€' = 199 998 bytes — just under the ceiling.
    const justUnder = '€'.repeat(66_666);
    expect(new TextEncoder().encode(justUnder).length).toBeLessThanOrEqual(200_000);
    expect(() => ApplicationTrackSchema.parse({ jobDescription: justUnder })).not.toThrow();
  });
});
