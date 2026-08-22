import { describe, expect, it } from 'vitest';

import { workTypeBadgeKey } from './work-type-badge';

describe('workTypeBadgeKey', () => {
  it('prefers the declared workType over the boolean remote flag', () => {
    expect(workTypeBadgeKey({ workType: 'hybrid', remote: false })).toBe('jobs.hybrid');
    expect(workTypeBadgeKey({ workType: 'on-site', remote: true })).toBe('jobs.onSite');
  });

  it('falls back to the remote flag when workType is absent (pre-field / boolean-only boards)', () => {
    expect(workTypeBadgeKey({ workType: undefined, remote: true })).toBe('jobs.remote');
  });

  it('renders nothing when neither is set', () => {
    expect(workTypeBadgeKey({ workType: undefined, remote: false })).toBeNull();
    expect(workTypeBadgeKey({ workType: undefined, remote: undefined })).toBeNull();
  });
});
