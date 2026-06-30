import { describe, expect, it } from 'vitest';

import { ROUTES } from '@/constants/routes';

import { parentRoute } from './parent-route';

describe('parentRoute', () => {
  it('maps application detail to ROUTES.APPLICATIONS', () => {
    expect(parentRoute('/applications/abc')).toBe(ROUTES.APPLICATIONS);
  });

  it('handles long ids', () => {
    expect(parentRoute('/applications/some-uuid-1234')).toBe(ROUTES.APPLICATIONS);
  });

  it('returns null for /applications (top-level)', () => {
    expect(parentRoute('/applications')).toBeNull();
  });

  it('returns null for /jobs', () => {
    expect(parentRoute('/jobs')).toBeNull();
  });

  it('returns null for root /', () => {
    expect(parentRoute('/')).toBeNull();
  });

  it('returns null for /settings', () => {
    expect(parentRoute('/settings')).toBeNull();
  });
});
