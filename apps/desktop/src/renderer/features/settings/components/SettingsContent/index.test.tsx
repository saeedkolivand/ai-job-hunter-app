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

import { TEST_IDS } from '@ajh/test-ids';

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
  GeneralSection: () => <div data-testid={TEST_IDS.settings.generalSection} />,
}));
vi.mock('@/features/settings/components/general-section/AppearanceCard', () => ({
  AppearanceCard: () => <div data-testid={TEST_IDS.settings.appearanceCard} />,
}));
vi.mock('@/features/settings/components/contact/ContactProfileTab', () => ({
  ContactProfileTab: () => <div data-testid={TEST_IDS.settings.contactTab} />,
}));
vi.mock('@/features/settings/components/ai-settings/AISettingsTab', () => ({
  AISettingsTab: () => <div data-testid={TEST_IDS.settings.aiTab} />,
}));
vi.mock('@/features/settings/components/preferences/OutputTonePreferences', () => ({
  OutputTonePreferences: () => <div data-testid={TEST_IDS.settings.tonePrefs} />,
}));
vi.mock('@/features/settings/components/preferences/JobLocationPreferences', () => ({
  JobLocationPreferences: () => <div data-testid={TEST_IDS.settings.jobLocation} />,
}));
vi.mock('@/features/settings/components/preferences/TechStackPreferences', () => ({
  TechStackPreferences: () => <div data-testid={TEST_IDS.settings.techStack} />,
}));
vi.mock('@/features/settings/components/preferences/AggregatorKeysSettings', () => ({
  AggregatorKeysSettings: () => <div data-testid={TEST_IDS.settings.aggregator} />,
}));
vi.mock('@/features/settings/components/preferences/AgencyCompaniesPreferences', () => ({
  AgencyCompaniesPreferences: () => <div data-testid="agency-companies-preferences" />,
}));
vi.mock('@/features/settings/components/preferences/ResumePreferences', () => ({
  ResumePreferences: () => <div data-testid={TEST_IDS.settings.resumePrefs} />,
}));
vi.mock('@/features/settings/components/accounts/AccountsSettingsTab', () => ({
  AccountsSettingsTab: () => <div data-testid={TEST_IDS.settings.accountsTab} />,
}));
vi.mock('@/features/settings/components/privacy/PrivacySettingsTab', () => ({
  PrivacySettingsTab: () => <div data-testid={TEST_IDS.settings.privacyTab} />,
}));
vi.mock('@/features/settings/components/preferences/PerformancePreferences', () => ({
  PerformancePreferences: () => <div data-testid={TEST_IDS.settings.perfPrefs} />,
}));
vi.mock('@/features/settings/components/preferences/DeveloperPreferences', () => ({
  DeveloperPreferences: () => <div data-testid={TEST_IDS.settings.devPrefs} />,
}));
vi.mock('@/features/settings/components/about/AboutTab', () => ({
  AboutTab: () => <div data-testid={TEST_IDS.settings.aboutTab} />,
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

/** Override window.matchMedia so `matches(query)` decides every media query. */
function stubMatchMedia(matches: (query: string) => boolean) {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: matches(query),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }) as unknown as MediaQueryList;
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

  it('scrolls with block: start and re-aligns once when the layout shifted it', () => {
    const rectSpy = vi
      .spyOn(Element.prototype, 'getBoundingClientRect')
      .mockImplementation(function (this: Element) {
        return { top: this.hasAttribute('data-settings-anchor') ? 300 : 0 } as DOMRect;
      });
    renderWithAnchor('performance-mode');
    // rAF fires synchronously in the effect — exactly one scroll before the settle re-check.
    expect(scrollSpy).toHaveBeenCalledOnce();
    expect(scrollSpy).toHaveBeenLastCalledWith(expect.objectContaining({ block: 'start' }));
    vi.advanceTimersByTime(500);
    expect(scrollSpy).toHaveBeenCalledTimes(2);
    rectSpy.mockRestore();
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
    stubMatchMedia((query) => query === '(prefers-reduced-motion: reduce)');
  });

  it('scrolls instantly, then consumes the anchor after the settle re-check (no pulse wait)', () => {
    const onConsumed = vi.fn();

    renderWithAnchor('performance-mode', onConsumed);

    expect(scrollSpy).toHaveBeenCalledOnce();
    expect(scrollSpy).toHaveBeenCalledWith(expect.objectContaining({ behavior: 'instant' }));
    vi.advanceTimersByTime(500);
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
    stubMatchMedia(() => false);
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

  it('still scrolls when the anchor mounts after the first frame', () => {
    const { container } = renderWithAnchor('late-anchor');
    expect(scrollSpy).not.toHaveBeenCalled();

    const late = document.createElement('div');
    late.setAttribute('data-settings-anchor', 'late-anchor');
    container.querySelector('.overflow-y-auto')?.appendChild(late);
    vi.advanceTimersByTime(400);

    expect(scrollSpy).toHaveBeenCalledOnce();
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
    stubMatchMedia(() => false);
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
