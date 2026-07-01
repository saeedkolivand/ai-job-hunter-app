import { describe, expect, it, vi } from 'vitest';

import {
  isKnownRoute,
  KNOWN_ROUTE_PATHS,
  resolveNotificationRoute,
  ROUTE_FALLBACK,
} from './notification-route';

// ── isKnownRoute (pure core) ──────────────────────────────────────────────────

describe('isKnownRoute', () => {
  const known = new Set(['/jobs', '/settings', '/']);

  it('returns true for a path in the set', () => {
    expect(isKnownRoute(known, '/jobs')).toBe(true);
  });

  it('returns true for root path', () => {
    expect(isKnownRoute(known, '/')).toBe(true);
  });

  it('returns false for an unknown path', () => {
    expect(isKnownRoute(known, '/nonexistent')).toBe(false);
  });

  it('returns false for an empty string', () => {
    expect(isKnownRoute(known, '')).toBe(false);
  });

  it('returns false for a partial match (prefix only)', () => {
    expect(isKnownRoute(known, '/job')).toBe(false);
  });
});

// ── resolveNotificationRoute ──────────────────────────────────────────────────

describe('resolveNotificationRoute', () => {
  it('passes through every known route unchanged', () => {
    for (const path of KNOWN_ROUTE_PATHS) {
      expect(resolveNotificationRoute(path)).toBe(path);
    }
  });

  it('returns ROUTE_FALLBACK for an unknown destination', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    expect(resolveNotificationRoute('/unknown-backend-route')).toBe(ROUTE_FALLBACK);
    warnSpy.mockRestore();
  });

  it('logs a console.warn for an unknown destination including the offending path', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    resolveNotificationRoute('/bogus');
    expect(warnSpy).toHaveBeenCalledWith(expect.stringContaining('/bogus'));
    warnSpy.mockRestore();
  });

  it('does NOT warn for a known route', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    resolveNotificationRoute('/jobs');
    expect(warnSpy).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it('ROUTE_FALLBACK is "/"', () => {
    expect(ROUTE_FALLBACK).toBe('/');
  });
});
