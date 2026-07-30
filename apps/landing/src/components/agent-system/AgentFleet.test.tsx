// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { AGENT_COUNT, STATIONS } from '@/data/agent-fleet';

import { AgentFleet } from './AgentFleet';

// jsdom provides neither matchMedia nor IntersectionObserver, and getBoundingClientRect
// is all-zero — see the repo lesson on jsdom zeroing layout geometry. Every effect
// under test here (useBeltScrub, useMapLinks, useReveal) reads matchMedia directly on
// mount, so it must be stubbed for ANY render of AgentFleet/BeltSection/FleetMapSection,
// not just the reveal-specific tests.
function stubMatchMedia(opts: { mobile?: boolean; reduce?: boolean } = {}) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: query.includes('prefers-reduced-motion')
      ? (opts.reduce ?? false)
      : (opts.mobile ?? false),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

// A synchronous, fully controllable IntersectionObserver fake — trigger() runs
// the real callback immediately instead of depending on real browser geometry.
class FakeIntersectionObserver {
  static instances: FakeIntersectionObserver[] = [];
  callback: IntersectionObserverCallback;
  observed: Element[] = [];
  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
    FakeIntersectionObserver.instances.push(this);
  }
  observe(el: Element) {
    this.observed.push(el);
  }
  unobserve(el: Element) {
    this.observed = this.observed.filter((e) => e !== el);
  }
  disconnect() {
    this.observed = [];
  }
  triggerAll() {
    const entries = this.observed.map(
      (target) => ({ target, isIntersecting: true }) as IntersectionObserverEntry
    );
    this.callback(entries, this as unknown as IntersectionObserver);
  }
  triggerOnly(el: Element) {
    this.callback(
      [{ target: el, isIntersecting: true } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver
    );
  }
}

let rafSpy: ReturnType<typeof vi.fn>;

beforeEach(() => {
  FakeIntersectionObserver.instances = [];
  rafSpy = vi.fn();
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('AgentFleet — composition', () => {
  it('renders all six sections, in order, as the characterization net for #916', () => {
    stubMatchMedia();
    const { container } = render(<AgentFleet />);

    expect(container.querySelector('.agent-fleet')).toBeTruthy();

    // Every h1 and every section heading (h2.section), across all six sections,
    // in document order. Belt renders its heading twice (sticky + vert layout).
    const headings = Array.from(container.querySelectorAll('h1, h2.section')).map(
      (el) => el.textContent
    );
    expect(headings).toEqual([
      'The Agent Fleet',
      'Intake → Delegation',
      'The Assembly Line',
      'The Assembly Line',
      'The Fleet',
      'Make Them Work Together',
      'With vs Without the Fleet',
    ]);
  });

  it('renders exactly one belt-title landmark, linked to .belt-section via aria-labelledby', () => {
    stubMatchMedia();
    const { container } = render(<AgentFleet />);

    const titles = container.querySelectorAll('#belt-title');
    expect(titles).toHaveLength(1);
    expect(container.querySelector('.belt-section')?.getAttribute('aria-labelledby')).toBe(
      'belt-title'
    );
  });

  it('renders each section root landmark exactly once', () => {
    stubMatchMedia();
    const { container } = render(<AgentFleet />);

    for (const selector of ['.intake', '.belt-section', '.map-wrap', '.big-prompt', '.versus']) {
      expect(container.querySelectorAll(selector)).toHaveLength(1);
    }
  });
});

describe('AgentFleet — data-driven rendering', () => {
  it('renders exactly STATIONS.length stations in both the horizontal and vertical belt layouts', () => {
    stubMatchMedia();
    const { container } = render(<AgentFleet />);

    expect(container.querySelectorAll('.belt-track > .station')).toHaveLength(STATIONS.length);
    expect(container.querySelectorAll('.belt-vert ol > li')).toHaveLength(STATIONS.length);
  });

  it('renders exactly AGENT_COUNT nodes on the fleet map, and displays AGENT_COUNT in the hero', () => {
    stubMatchMedia();
    const { container } = render(<AgentFleet />);

    expect(container.querySelectorAll('.constellation .node')).toHaveLength(AGENT_COUNT);
    expect(container.querySelector('[data-count]')?.textContent).toBe(String(AGENT_COUNT));
  });
});

describe('AgentFleet — useReveal', () => {
  it('adds "in" to every .reveal element across sections once IntersectionObserver reports it visible', () => {
    stubMatchMedia({ reduce: false });
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
    vi.stubGlobal('requestAnimationFrame', rafSpy);

    const { container } = render(<AgentFleet />);
    const revealEls = Array.from(container.querySelectorAll('.reveal'));
    // Sanity: every non-belt section contributes at least one .reveal element,
    // so this genuinely spans the tree (root ref, children markup).
    expect(revealEls.length).toBeGreaterThan(10);
    for (const el of revealEls) expect(el.classList.contains('in')).toBe(false);

    const io = FakeIntersectionObserver.instances[0];
    if (!io) throw new Error('expected useReveal to construct an IntersectionObserver');
    io.triggerAll();

    for (const el of revealEls) expect(el.classList.contains('in')).toBe(true);
    // Non-reduced motion animates the hero count-up via requestAnimationFrame.
    expect(rafSpy).toHaveBeenCalled();
  });

  it('respects prefers-reduced-motion: the hero count-up sets its final value synchronously, no rAF', () => {
    stubMatchMedia({ reduce: true });
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver);
    vi.stubGlobal('requestAnimationFrame', rafSpy);

    const { container } = render(<AgentFleet />);
    const countEl = container.querySelector<HTMLElement>('[data-count]');
    if (!countEl) throw new Error('expected a [data-count] element');

    const io = FakeIntersectionObserver.instances[0];
    if (!io) throw new Error('expected useReveal to construct an IntersectionObserver');
    io.triggerOnly(countEl);

    expect(countEl.textContent).toBe(String(AGENT_COUNT));
    expect(countEl.classList.contains('in')).toBe(true);
    expect(rafSpy).not.toHaveBeenCalled();
  });
});
