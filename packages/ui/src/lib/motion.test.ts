import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  prefersReducedMotion,
  resolveTransition,
  staggeredItem,
  transition,
  withDelay,
} from './motion';

describe('staggeredItem', () => {
  it('scales the delay with the index', () => {
    expect(staggeredItem(0).delay).toBe(0);
    expect(staggeredItem(5).delay).toBeCloseTo(0.05);
  });

  it('caps the delay at maxDelay', () => {
    expect(staggeredItem(1000).delay).toBe(0.2);
    expect(staggeredItem(1000, 0.5).delay).toBe(0.5);
  });
});

describe('withDelay', () => {
  it('wraps transition.normal with an explicit delay', () => {
    const t = withDelay(0.15);
    expect(t.delay).toBe(0.15);
    expect(t.duration).toBe(transition.normal.duration);
    expect(t.ease).toBe(transition.normal.ease);
  });
});

describe('prefersReducedMotion', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('returns false when the media query does not match', () => {
    vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: false,
    } as unknown as MediaQueryList);
    expect(prefersReducedMotion()).toBe(false);
  });

  it('returns true when the user prefers reduced motion', () => {
    vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: true,
    } as unknown as MediaQueryList);
    expect(prefersReducedMotion()).toBe(true);
  });
});

describe('resolveTransition', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('passes the transition through when motion is allowed', () => {
    vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: false,
    } as unknown as MediaQueryList);
    expect(resolveTransition(transition.normal)).toBe(transition.normal);
  });

  it('returns an instant transition under reduced motion', () => {
    vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: true,
    } as unknown as MediaQueryList);
    expect(resolveTransition(transition.normal)).toEqual({ duration: 0 });
  });
});
