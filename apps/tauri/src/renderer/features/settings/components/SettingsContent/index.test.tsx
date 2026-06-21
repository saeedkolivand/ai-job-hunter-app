/**
 * SettingsContent — pendingAnchor pulse-effect tests (advisory #3).
 *
 * Covers:
 *  - (normal motion) rAF fires → scrollIntoView called + pulse classes added;
 *    after PULSE_DURATION the classes are removed and onAnchorConsumed called once.
 *  - (reduced motion) matchMedia prefers-reduced-motion → scrollIntoView called
 *    instantly + onAnchorConsumed called immediately, no ring classes.
 *  - Cleanup: unmounting before PULSE_DURATION fires cancels the timer.
 *  - No pendingAnchor → no scrollIntoView, no onAnchorConsumed.
 *
 * Key jsdom constraint: React sets scrollRef.current to the component's own
 * <div ref={scrollRef}> — any manually-built ref pointing at a document.body
 * div is overwritten. Tests must therefore assert via Element.prototype.scrollIntoView
 * spy (called on whatever element the component finds) rather than a per-element spy.
 *
 * rAF: vi.useFakeTimers() MUST be called before vi.stubGlobal('requestAnimationFrame')
 * — useFakeTimers overwrites rAF even when not listed in `toFake`. Reversed order
 * causes the sync stub to be overwritten, and the rAF callback never fires.
 */

import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

// ── matchMedia leak guard ─────────────────────────────────────────────────────
// Several describe blocks override window.matchMedia in their beforeEach to
// simulate prefers-reduced-motion.  Save the original once here and restore it
// in afterEach so the override never leaks into a sibling suite.
const originalMatchMedia = window.matchMedia;

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Stub every section component so SettingsContent renders without IPC ───────

vi.mock('@/features/settings/components/general-section', () => ({
  GeneralSection: () => <div data-testid="general-section" />,
}));
vi.mock('@/features/settings/components/general-section/AppearanceCard', () => ({
  AppearanceCard: () => <div data-testid="appearance-card" />,
}));
vi.mock('@/features/settings/components/contact/ContactProfileTab', () => ({
  ContactProfileTab: () => <div data-testid="contact-tab" />,
}));
vi.mock('@/features/settings/components/ai-settings/AISettingsTab', () => ({
  AISettingsTab: () => <div data-testid="ai-tab" />,
}));
vi.mock('@/features/settings/components/preferences/OutputTonePreferences', () => ({
  OutputTonePreferences: () => <div data-testid="tone-prefs" />,
}));
vi.mock('@/features/settings/components/preferences/JobLocationPreferences', () => ({
  JobLocationPreferences: () => <div data-testid="job-location" />,
}));
vi.mock('@/features/settings/components/preferences/TechStackPreferences', () => ({
  TechStackPreferences: () => <div data-testid="tech-stack" />,
}));
vi.mock('@/features/settings/components/preferences/AggregatorKeysSettings', () => ({
  AggregatorKeysSettings: () => <div data-testid="aggregator" />,
}));
vi.mock('@/features/settings/components/preferences/ResumePreferences', () => ({
  ResumePreferences: () => <div data-testid="resume-prefs" />,
}));
vi.mock('@/features/settings/components/accounts/AccountsSettingsTab', () => ({
  AccountsSettingsTab: () => <div data-testid="accounts-tab" />,
}));
vi.mock('@/features/settings/components/privacy/PrivacySettingsTab', () => ({
  PrivacySettingsTab: () => <div data-testid="privacy-tab" />,
}));
vi.mock('@/features/settings/components/preferences/PerformancePreferences', () => ({
  PerformancePreferences: () => <div data-testid="perf-prefs" />,
}));
vi.mock('@/features/settings/components/preferences/DeveloperPreferences', () => ({
  DeveloperPreferences: () => <div data-testid="dev-prefs" />,
}));
vi.mock('@/features/settings/components/about/AboutTab', () => ({
  AboutTab: () => <div data-testid="about-tab" />,
}));

// ── component under test ──────────────────────────────────────────────────────

import { NAV_GROUPS, type NavItem } from '@/features/settings/constants';

import { SettingsContent } from './index';

// ── fixtures ──────────────────────────────────────────────────────────────────

const PULSE_DURATION = 1500; // mirrors the constant in the component

const _perfGroup = NAV_GROUPS[1];
const _perfItem = _perfGroup?.items[2];
if (!_perfGroup || !_perfItem)
  throw new Error('NAV_GROUPS[1].items[2] not found — fixture out of sync');
const performanceNavItem: NavItem = _perfItem;

const _prefGroup = NAV_GROUPS[0];
const _aiItem = _prefGroup?.items[3];
const _jobItem = _prefGroup?.items[4];
if (!_prefGroup || !_aiItem || !_jobItem)
  throw new Error('NAV_GROUPS[0].items[3/4] not found — fixture out of sync');
const aiNavItem: NavItem = _aiItem;
const jobNavItem: NavItem = _jobItem;

// ── rAF helper ────────────────────────────────────────────────────────────────
//
// vi.useFakeTimers() overwrites requestAnimationFrame even when not listed in
// `toFake`. So: call useFakeTimers FIRST, then stubGlobal rAF AFTER.

function installSyncRaf() {
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    cb(performance.now());
    return 0;
  });
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
}

/**
 * Render SettingsContent for the performance section with a pendingAnchor.
 * React sets scrollRef.current to the component's own inner div (via the ref
 * prop on that div), so the component-rendered DOM contains the anchor element.
 */
function renderWithAnchor(anchor: string | null, onConsumed = vi.fn()) {
  const { container, unmount } = render(
    <SettingsContent
      activeSection={'performance'}
      current={performanceNavItem}
      localName="Test"
      setLocalName={vi.fn()}
      setUserName={vi.fn()}
      userName="Test"
      pendingAnchor={anchor}
      scrollRef={{ current: null }}
      onAnchorConsumed={onConsumed}
    />
  );
  return { container, unmount, onConsumed };
}

// ── cleanup ───────────────────────────────────────────────────────────────────

afterEach(() => {
  vi.useRealTimers();
  // Restore matchMedia after every test so describe-block overrides don't leak
  // into sibling suites.  Do NOT call vi.restoreAllMocks() here — the
  // scrollIntoView spy is managed per describe-block via beforeAll/afterAll +
  // mockClear; restoreAllMocks would restore the prototype mid-block, breaking
  // the mockClear isolation strategy.
  window.matchMedia = originalMatchMedia;
  document.body.innerHTML = '';
});

// ─────────────────────────────────────────────────────────────────────────────
// Normal motion (prefers-reduced-motion: false — default in vitest.setup.ts)
// ─────────────────────────────────────────────────────────────────────────────

describe('SettingsContent — pendingAnchor, normal motion', () => {
  // Install the spy once for the block; clear (not restore) between tests to
  // avoid stacking spy wrappers (vi.spyOn + vi.restoreAllMocks + vi.spyOn stacks).
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeAll(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(vi.fn());
  });

  afterAll(() => {
    scrollSpy.mockRestore();
  });

  beforeEach(() => {
    scrollSpy.mockClear();
    // CRITICAL: fake timers FIRST, then sync rAF stub (else useFakeTimers overwrites it)
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    installSyncRaf();
  });

  it('scrollIntoView is called on the anchored element after rAF', () => {
    renderWithAnchor('performance-mode');
    // rAF fires synchronously in the effect — scrollIntoView must be called exactly once.
    expect(scrollSpy).toHaveBeenCalledOnce();
  });

  it('pulse classes are added to the anchored element after rAF (before PULSE_DURATION)', () => {
    const { container } = renderWithAnchor('performance-mode');

    // The anchor element the component finds via querySelector('[data-settings-anchor]')
    const anchor = container.querySelector('[data-settings-anchor="performance-mode"]');
    if (!anchor) throw new Error('anchor element not found in rendered DOM');
    expect(anchor.classList.contains('ring-2')).toBe(true);
    expect(anchor.classList.contains('ring-brand')).toBe(true);
  });

  it('pulse classes are removed and onAnchorConsumed called after PULSE_DURATION', () => {
    const onConsumed = vi.fn();
    const { container } = renderWithAnchor('performance-mode', onConsumed);

    const anchor = container.querySelector('[data-settings-anchor="performance-mode"]');
    if (!anchor) throw new Error('anchor element not found in rendered DOM');

    // Advance past PULSE_DURATION to fire the cleanup timer.
    vi.advanceTimersByTime(PULSE_DURATION + 10);

    expect(onConsumed).toHaveBeenCalledOnce();
    expect(anchor.classList.contains('ring-2')).toBe(false);
    expect(anchor.classList.contains('ring-brand')).toBe(false);
    expect(anchor.classList.contains('rounded-xl')).toBe(false);
    expect(anchor.classList.contains('transition-[box-shadow]')).toBe(false);
  });

  it('cleanup on unmount cancels the timer so onAnchorConsumed is never called', () => {
    const onConsumed = vi.fn();
    const { container, unmount } = renderWithAnchor('performance-mode', onConsumed);

    const anchor = container.querySelector('[data-settings-anchor="performance-mode"]');
    if (!anchor) throw new Error('anchor element not found in rendered DOM');
    expect(anchor.classList.contains('ring-2')).toBe(true);

    // Unmount before the timer fires.
    unmount();
    vi.advanceTimersByTime(PULSE_DURATION + 100);

    expect(onConsumed).not.toHaveBeenCalled();
  });

  it('no scrollIntoView or onAnchorConsumed when pendingAnchor is null', () => {
    const onConsumed = vi.fn();

    renderWithAnchor(null, onConsumed);

    vi.advanceTimersByTime(PULSE_DURATION + 100);
    expect(onConsumed).not.toHaveBeenCalled();
    expect(scrollSpy).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Reduced motion (prefers-reduced-motion: reduce)
// ─────────────────────────────────────────────────────────────────────────────

describe('SettingsContent — pendingAnchor, reduced motion', () => {
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeAll(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(vi.fn());
  });

  afterAll(() => {
    scrollSpy.mockRestore();
  });

  beforeEach(() => {
    scrollSpy.mockClear();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    installSyncRaf();
    // Override matchMedia to return matches=true for the reduced-motion query only.
    window.matchMedia = (query: string): MediaQueryList => ({
      matches: query === '(prefers-reduced-motion: reduce)',
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    });
  });

  it('scrollIntoView is called with behavior: instant (not smooth)', () => {
    const onConsumed = vi.fn();

    renderWithAnchor('performance-mode', onConsumed);

    expect(scrollSpy).toHaveBeenCalledOnce();
    const callArg = scrollSpy.mock.calls[0]?.[0] as ScrollIntoViewOptions | undefined;
    expect(callArg?.behavior).toBe('instant');
  });

  it('onAnchorConsumed called immediately (inside rAF), no PULSE_DURATION wait', () => {
    const onConsumed = vi.fn();

    renderWithAnchor('performance-mode', onConsumed);

    // rAF is sync — consumed without advancing timers.
    expect(onConsumed).toHaveBeenCalledOnce();
  });

  it('no pulse ring classes are added in reduced-motion mode', () => {
    const { container } = renderWithAnchor('performance-mode');

    const anchor = container.querySelector('[data-settings-anchor="performance-mode"]');
    if (!anchor) throw new Error('anchor element not found in rendered DOM');
    expect(anchor.classList.contains('ring-2')).toBe(false);
    expect(anchor.classList.contains('ring-brand')).toBe(false);
    expect(anchor.classList.contains('rounded-xl')).toBe(false);
    expect(anchor.classList.contains('transition-[box-shadow]')).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// querySelector-null defensive path
// When pendingAnchor names an element that is absent from the rendered section,
// querySelector returns null and the effect early-returns. No throw, no pulse
// classes, onAnchorConsumed never called.
// ─────────────────────────────────────────────────────────────────────────────

describe('SettingsContent — querySelector-null defensive path', () => {
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeAll(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(vi.fn());
  });

  afterAll(() => {
    scrollSpy.mockRestore();
  });

  beforeEach(() => {
    scrollSpy.mockClear();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    installSyncRaf();
    // Ensure normal-motion context (reduced-motion describe block's beforeEach may
    // have left matchMedia returning matches=true for the reduced-motion query).
    window.matchMedia = (query: string): MediaQueryList => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    });
  });

  it('does not throw, does not call onAnchorConsumed, and applies no pulse classes when the anchor element is absent', () => {
    const onConsumed = vi.fn();

    // Render the 'job' section (anchors: job-location, job-techstack, job-aggregator).
    // pendingAnchor 'ai-provider' does NOT exist in this section's DOM.
    const { container } = render(
      <SettingsContent
        activeSection={'job'}
        current={jobNavItem}
        localName="Test"
        setLocalName={vi.fn()}
        setUserName={vi.fn()}
        userName="Test"
        pendingAnchor={'ai-provider'}
        scrollRef={{ current: null }}
        onAnchorConsumed={onConsumed}
      />
    );

    // Advance past PULSE_DURATION to confirm the timer branch is not involved either.
    vi.advanceTimersByTime(PULSE_DURATION + 10);

    // Effect must have early-returned: onAnchorConsumed never called.
    expect(onConsumed).not.toHaveBeenCalled();
    // No scrollIntoView on anything.
    expect(scrollSpy).not.toHaveBeenCalled();
    // No pulse classes on any element in the container.
    const allElements = container.querySelectorAll('*');
    allElements.forEach((el) => {
      expect(el.classList.contains('ring-2')).toBe(false);
      expect(el.classList.contains('ring-brand')).toBe(false);
    });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// activeSection dependency re-fires the effect
// Simulates a search result in a DIFFERENT section: initial render with
// activeSection='job' makes querySelector miss 'ai-provider' (absent from job
// DOM). After rerender with activeSection='ai' the anchor is present and the
// effect re-fires, applying the pulse. Guards the activeSection dep-array entry.
// ─────────────────────────────────────────────────────────────────────────────

describe('SettingsContent — activeSection change re-fires pulse effect', () => {
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeAll(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(vi.fn());
  });

  afterAll(() => {
    scrollSpy.mockRestore();
  });

  beforeEach(() => {
    scrollSpy.mockClear();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    installSyncRaf();
    // Ensure normal-motion context (matchMedia may have been left in reduced-motion
    // state by a sibling describe block's beforeEach that runs before ours).
    window.matchMedia = (query: string): MediaQueryList => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    });
  });

  it('fires scroll+pulse on the anchor element after activeSection changes to the section that owns it', () => {
    const onConsumed = vi.fn();

    const sharedProps = {
      localName: 'Test',
      setLocalName: vi.fn(),
      setUserName: vi.fn(),
      userName: 'Test',
      pendingAnchor: 'ai-provider' as const,
      scrollRef: { current: null },
      onAnchorConsumed: onConsumed,
    };

    // First render: job section — 'ai-provider' not present, effect early-returns.
    const { container, rerender } = render(
      <SettingsContent activeSection={'job'} current={jobNavItem} {...sharedProps} />
    );
    expect(onConsumed).not.toHaveBeenCalled();
    expect(scrollSpy).not.toHaveBeenCalled();

    // Rerender with activeSection='ai' — 'ai-provider' anchor now exists in the DOM.
    rerender(<SettingsContent activeSection={'ai'} current={aiNavItem} {...sharedProps} />);

    // Effect re-ran because activeSection changed: scroll must have fired.
    expect(scrollSpy).toHaveBeenCalledOnce();

    // Pulse classes applied to the anchor element.
    const anchor = container.querySelector('[data-settings-anchor="ai-provider"]');
    if (!anchor)
      throw new Error('[data-settings-anchor="ai-provider"] not in DOM after section switch');
    expect(anchor.classList.contains('ring-2')).toBe(true);
    expect(anchor.classList.contains('ring-brand')).toBe(true);

    // Advance past PULSE_DURATION — cleanup fires, onAnchorConsumed called once.
    vi.advanceTimersByTime(PULSE_DURATION + 10);
    expect(onConsumed).toHaveBeenCalledOnce();
    expect(anchor.classList.contains('ring-2')).toBe(false);
  });
});
