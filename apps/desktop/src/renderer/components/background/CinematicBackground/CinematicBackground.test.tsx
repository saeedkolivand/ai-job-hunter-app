/**
 * CinematicBackground — layer presence/absence tests.
 *
 * jsdom's cssstyle drops hex/color-mix linear-gradient / radial-gradient inline
 * styles, so we NEVER assert on computed gradient CSS. Instead we query stable
 * class-name seams that the component guarantees:
 *   - Aurora ribbons: `animate-aurora-1`, `animate-aurora-2`, `animate-aurora-3`
 *   - Nebulae:        `animate-nebula-1`, `animate-nebula-2`
 *   - Cursor glow:    `cursor-glow`
 *
 * We also assert on RAF / pointer-listener gating by spying on
 * `window.addEventListener` and `requestAnimationFrame`.
 *
 * Mock strategy: `useResolvedPerformanceProfile` is mocked at the module level
 * so tests can feed any profile without a full store/provider setup.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

import type { PerformanceProfile } from '@/store/preferences-schema';

// ── mock useResolvedPerformanceProfile ────────────────────────────────────────

const mockProfile = vi.fn(() => ({}) as PerformanceProfile);

vi.mock('@/store/preferences-store', () => ({
  useResolvedPerformanceProfile: () => mockProfile(),
}));

// ── import component AFTER mocks ──────────────────────────────────────────────

import { CinematicBackground } from './index';

// ── profile helpers ───────────────────────────────────────────────────────────

function makeProfile(
  overrides: Partial<PerformanceProfile['visual']> & {
    backend?: Partial<PerformanceProfile['backend']>;
  } = {}
): PerformanceProfile {
  const { backend, ...visualOverrides } = overrides;
  return {
    visual: {
      aurora: false,
      nebula: false,
      richNebula: false,
      cursorGlow: false,
      blur: 'full',
      animations: true,
      ...visualOverrides,
    },
    backend: {
      concurrency: 'balanced',
      keepAlive: 'balanced',
      cache: 'balanced',
      ...backend,
    },
  };
}

const ALL_OFF = makeProfile(); // aurora=false, nebula=false, cursorGlow=false

// ── test helpers ──────────────────────────────────────────────────────────────

function auroraNodes(container: HTMLElement): NodeListOf<Element> {
  return container.querySelectorAll('.animate-aurora-1, .animate-aurora-2, .animate-aurora-3');
}

function nebulaNodes(container: HTMLElement): NodeListOf<Element> {
  return container.querySelectorAll('.animate-nebula-1, .animate-nebula-2');
}

function cursorGlowNodes(container: HTMLElement): NodeListOf<Element> {
  return container.querySelectorAll('.cursor-glow');
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('CinematicBackground — layer presence/absence', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('all-visual-off profile (low-memory equivalent)', () => {
    it('renders nothing when all layers are off', () => {
      mockProfile.mockReturnValue(ALL_OFF);
      const { container } = render(<CinematicBackground />);
      expect(container.firstChild).toBeNull();
    });
  });

  describe('aurora-only profile', () => {
    it('renders 3 aurora ribbon nodes and no nebula or cursor-glow nodes', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true }));
      const { container } = render(<CinematicBackground />);

      expect(auroraNodes(container)).toHaveLength(3);
      expect(nebulaNodes(container)).toHaveLength(0);
      expect(cursorGlowNodes(container)).toHaveLength(0);
    });
  });

  describe('nebula rendering', () => {
    it('renders exactly ONE nebula node when nebula=true and richNebula=false', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true, nebula: true, richNebula: false }));
      const { container } = render(<CinematicBackground />);

      const nebulae = nebulaNodes(container);
      expect(nebulae).toHaveLength(1);
      // The single nebula must be the first one (animate-nebula-1).
      const first = nebulae[0];
      if (!first) throw new Error('expected exactly one nebula node');
      expect(first.classList.contains('animate-nebula-1')).toBe(true);
    });

    it('renders TWO nebula nodes when nebula=true and richNebula=true', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true, nebula: true, richNebula: true }));
      const { container } = render(<CinematicBackground />);

      expect(nebulaNodes(container)).toHaveLength(2);
    });

    it('renders no nebula nodes when nebula=false even with richNebula=true', () => {
      // richNebula without nebula — showRichNebula = nebula && richNebula = false.
      mockProfile.mockReturnValue(makeProfile({ nebula: false, richNebula: true, aurora: true }));
      const { container } = render(<CinematicBackground />);
      expect(nebulaNodes(container)).toHaveLength(0);
    });
  });

  describe('cursor glow RAF / listener gating', () => {
    const originalMatchMedia = window.matchMedia;
    beforeEach(() => {
      // Ensure matchMedia returns non-reduced-motion so the full RAF path runs.
      const stub = {
        matches: false,
        media: '',
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      } as unknown as MediaQueryList;
      window.matchMedia = (_query: string) => stub;
    });
    afterEach(() => {
      window.matchMedia = originalMatchMedia;
    });

    it('no pointermove listener and no RAF when cursorGlow=false', () => {
      mockProfile.mockReturnValue(makeProfile({ cursorGlow: false }));
      const addEventSpy = vi.spyOn(window, 'addEventListener');
      const rafSpy = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(0);

      render(<CinematicBackground />);

      const pointermoveCalls = addEventSpy.mock.calls.filter((args) => args[0] === 'pointermove');
      expect(pointermoveCalls).toHaveLength(0);
      expect(rafSpy).not.toHaveBeenCalled();
    });

    it('schedules RAF when cursorGlow=true (no reduced-motion)', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true, cursorGlow: true }));
      const rafSpy = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);

      render(<CinematicBackground />);

      expect(rafSpy).toHaveBeenCalled();
    });

    it('adds a pointermove listener when cursorGlow=true (no reduced-motion)', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true, cursorGlow: true }));
      const addEventSpy = vi.spyOn(window, 'addEventListener');
      vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);

      render(<CinematicBackground />);

      const pointermoveCalls = addEventSpy.mock.calls.filter((args) => args[0] === 'pointermove');
      expect(pointermoveCalls.length).toBeGreaterThan(0);
    });

    it('renders the cursor-glow node when cursorGlow=true', () => {
      mockProfile.mockReturnValue(makeProfile({ aurora: true, cursorGlow: true }));
      vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(1);

      const { container } = render(<CinematicBackground />);
      expect(cursorGlowNodes(container)).toHaveLength(1);
    });
  });
});
