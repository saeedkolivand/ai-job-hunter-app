import { describe, expect, it } from 'vitest';

import guard from './guard-empty-release-notes.cjs';

// Silent logger so the "notes present" confirmation doesn't spam test output.
const logger = { log() {} };

describe('guard-empty-release-notes prepare hook', () => {
  it('throws when release notes are an empty string', () => {
    expect(() => guard.prepare({}, { nextRelease: { notes: '' }, logger })).toThrow(/empty/i);
  });

  it('throws when release notes are whitespace-only', () => {
    expect(() => guard.prepare({}, { nextRelease: { notes: '  \n\t ' }, logger })).toThrow(
      /conventional-changelog-conventionalcommits/
    );
  });

  it('throws when nextRelease.notes is missing', () => {
    expect(() => guard.prepare({}, { nextRelease: {}, logger })).toThrow();
  });

  it('does not throw when release notes are present', () => {
    expect(() =>
      guard.prepare({}, { nextRelease: { notes: '## x', version: '1.2.3' }, logger })
    ).not.toThrow();
  });
});
