/**
 * AppSplash — unit tests
 *
 * Covers:
 *   1. Wordmark is rendered on mount.
 *   2. Overlay unmounts (is hidden) after the MIN_DISPLAY_MS timer fires (fake timers).
 *   3. Shimmer bar is present in the normal path.
 *   4. Reduced-motion path: shimmer bar is absent; overlay still dismisses.
 *   5. pointer-events-none applied to the wrapper the moment visible flips false.
 *   6. Hard fallback unmount: wrapper is fully removed after MIN_DISPLAY_MS + EXIT_BUDGET_MS
 *      even without onAnimationComplete firing (WAAPI stripped in jsdom shim).
 *
 * Motion/react is globally shimmed in vitest.setup.ts — AnimatePresence renders
 * children synchronously and exit animations are instant, so `exit` behaviour is
 * observable without real WAAPI.
 *
 * matchMedia is globally shimmed in vitest.setup.ts (matches: false = no
 * reduced-motion). Per-test overrides replace it where needed.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── component under test (import AFTER mocks) ─────────────────────────────────
import { AppSplash } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function makeMatchMedia(reducedMotion: boolean) {
  return (query: string): MediaQueryList => ({
    matches: query.includes('prefers-reduced-motion') ? reducedMotion : false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AppSplash — wordmark', () => {
  it('renders the app title on initial mount', () => {
    render(<AppSplash />);
    // i18n mock returns the key — 'app.title' is the expected text.
    expect(screen.getByText('app.title')).toBeInTheDocument();
  });

  it('renders the tagline on initial mount', () => {
    render(<AppSplash />);
    expect(screen.getByText('app.tagline')).toBeInTheDocument();
  });
});

describe('AppSplash — timer dismissal', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('is visible before 700 ms have elapsed', () => {
    render(<AppSplash />);
    act(() => {
      vi.advanceTimersByTime(699);
    });
    expect(screen.queryByRole('status')).toBeInTheDocument();
  });

  it('is hidden (unmounted) after 700 ms have elapsed', () => {
    render(<AppSplash />);
    act(() => {
      vi.advanceTimersByTime(700);
    });
    // AnimatePresence shim removes the element immediately on exit in jsdom.
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});

describe('AppSplash — pointer-events hardening', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('applies pointer-events-none to the wrapper once visible flips false', () => {
    const { container } = render(<AppSplash />);
    act(() => {
      vi.advanceTimersByTime(700);
    });
    // The inner role="status" element is gone; the outer wrapper still exists
    // but must carry pointer-events-none so mid-fade clicks are not intercepted.
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toBeInTheDocument();
    expect(wrapper.className).toContain('pointer-events-none');
  });

  it('fully unmounts (returns null) after MIN_DISPLAY_MS + EXIT_BUDGET_MS fallback', () => {
    // jsdom shim strips onAnimationComplete, so the hard fallback setTimeout
    // (EXIT_BUDGET_MS = 500 ms after dismiss) is the only removal path.
    // Two separate act() calls so React flushes the visible→false state update
    // and registers the fallback effect before we advance the remaining 500 ms.
    const { container } = render(<AppSplash />);
    act(() => {
      vi.advanceTimersByTime(700);
    }); // visible→false; fallback timer scheduled
    act(() => {
      vi.advanceTimersByTime(500);
    }); // fallback fires → mounted→false → null
    // Component returns null — container should be empty.
    expect(container.firstElementChild).toBeNull();
  });

  it('does NOT apply pointer-events-none before the dismiss timer fires', () => {
    const { container } = render(<AppSplash />);
    act(() => {
      vi.advanceTimersByTime(699);
    });
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper?.className ?? '').not.toContain('pointer-events-none');
  });
});

describe('AppSplash — shimmer loader (normal motion)', () => {
  beforeEach(() => {
    // Ensure reduced-motion is OFF for this suite.
    window.matchMedia = makeMatchMedia(false);
  });

  it('renders the shimmer bar when prefers-reduced-motion is false', () => {
    const { container } = render(<AppSplash />);
    // aria-hidden shimmer bar — query by class presence.
    const shimmer = container.querySelector('.bg-brand-gradient');
    expect(shimmer).toBeInTheDocument();
  });
});

describe('AppSplash — reduced-motion path', () => {
  beforeEach(() => {
    window.matchMedia = makeMatchMedia(true);
  });

  it('does NOT render the shimmer bar when prefers-reduced-motion is true', () => {
    const { container } = render(<AppSplash />);
    const shimmer = container.querySelector('.bg-brand-gradient');
    expect(shimmer).not.toBeInTheDocument();
  });

  it('still renders the wordmark under reduced-motion', () => {
    render(<AppSplash />);
    expect(screen.getByText('app.title')).toBeInTheDocument();
  });

  it('still dismisses after the timer under reduced-motion', () => {
    vi.useFakeTimers();
    try {
      render(<AppSplash />);
      act(() => {
        vi.advanceTimersByTime(700);
      });
      expect(screen.queryByRole('status')).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });
});
