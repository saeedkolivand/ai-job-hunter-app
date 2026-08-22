import { describe, expect, it } from 'vitest';

import { getWorkTypeBadge } from './work-type-badge';

describe('getWorkTypeBadge', () => {
  it('prefers the declared workType over the boolean remote flag', () => {
    expect(getWorkTypeBadge({ workType: 'hybrid', remote: false })).toEqual({
      key: 'jobs.hybrid',
      color: 'cyan',
    });
    expect(getWorkTypeBadge({ workType: 'on-site', remote: true })).toEqual({
      key: 'jobs.onSite',
      color: 'default',
    });
    expect(getWorkTypeBadge({ workType: 'remote', remote: false })).toEqual({
      key: 'jobs.remote',
      color: 'green',
    });
  });

  it('falls back to the remote flag when workType is absent (pre-field / boolean-only boards)', () => {
    expect(getWorkTypeBadge({ workType: undefined, remote: true })).toEqual({
      key: 'jobs.remote',
      color: 'green',
    });
  });

  it('renders nothing when neither is set', () => {
    expect(getWorkTypeBadge({ workType: undefined, remote: false })).toBeNull();
    expect(getWorkTypeBadge({ workType: undefined, remote: undefined })).toBeNull();
  });

  it('never resolves a non-declared key to the object prototype (Map lookup, not object index)', () => {
    // A `WorkTypeOption` is a closed union at the type level, but the lookup
    // itself must still be safe against an unexpected runtime value — assert
    // the mechanism directly rather than trusting the type system alone.
    const hostile = { workType: 'constructor', remote: false } as unknown as Parameters<
      typeof getWorkTypeBadge
    >[0];
    expect(getWorkTypeBadge(hostile)).toBeNull();
  });
});
