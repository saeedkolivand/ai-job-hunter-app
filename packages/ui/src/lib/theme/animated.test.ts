import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { applyThemeAnimated } from '../theme';
import {
  basePrefs as prefs,
  resetThemeEnv,
  restoreThemeEnv,
  setStartViewTransition,
  stubMatchMedia,
} from './test-support';

describe('applyThemeAnimated — view-transition gating', () => {
  beforeEach(resetThemeEnv);

  afterEach(() => {
    setStartViewTransition(undefined);
    restoreThemeEnv();
  });

  it('calls startViewTransition on non-Linux UA when the API is present', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' });
    const spy = vi.fn((cb: () => void) => {
      cb();
    });
    setStartViewTransition(spy);

    applyThemeAnimated(prefs);

    expect(spy).toHaveBeenCalledOnce();
    // Theme must still apply (data-color-scheme written inside the callback).
    expect(document.documentElement.dataset.colorScheme).toBe('dark');
  });

  it('does NOT call startViewTransition on Linux UA — applies theme directly', () => {
    vi.stubGlobal('navigator', {
      userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15',
    });
    const spy = vi.fn();
    setStartViewTransition(spy);

    applyThemeAnimated(prefs);

    expect(spy).not.toHaveBeenCalled();
    // Theme still applied directly.
    expect(document.documentElement.dataset.colorScheme).toBe('dark');
  });

  it('applies theme directly when startViewTransition is absent (older browsers)', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' });
    // Ensure startViewTransition is not present.
    setStartViewTransition(undefined);

    applyThemeAnimated(prefs);

    expect(document.documentElement.dataset.colorScheme).toBe('dark');
  });

  it('skips startViewTransition when prefers-reduced-motion is set', () => {
    vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)' });
    stubMatchMedia({ '(prefers-reduced-motion: reduce)': true });
    const spy = vi.fn();
    setStartViewTransition(spy);

    applyThemeAnimated(prefs);

    expect(spy).not.toHaveBeenCalled();
    expect(document.documentElement.dataset.colorScheme).toBe('dark');
  });
});
