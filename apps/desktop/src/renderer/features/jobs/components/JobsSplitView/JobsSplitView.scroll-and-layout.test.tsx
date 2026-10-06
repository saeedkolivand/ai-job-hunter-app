/**
 * JobsSplitView — list scroll restore/persist, and the responsive width
 * contract (narrow-window regression).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';

import { renderSplit, resetSplit } from './split-harness';

beforeEach(resetSplit);

// jsdom has no layout engine — scrollTop getter always returns 0 regardless of
// assignments. Spy on the Element.prototype setter instead so we can observe
// that the mount effect actually wrote the stored value.
describe('JobsSplitView — scroll restore on mount', () => {
  let scrollTopWrites: number[];
  let savedScrollTopDescriptor: PropertyDescriptor | undefined;

  beforeEach(() => {
    scrollTopWrites = [];
    savedScrollTopDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollTop');
    Object.defineProperty(Element.prototype, 'scrollTop', {
      set(v: number) {
        scrollTopWrites.push(v);
      },
      get() {
        return 0;
      },
      configurable: true,
    });
  });

  afterEach(() => {
    if (savedScrollTopDescriptor) {
      Object.defineProperty(Element.prototype, 'scrollTop', savedScrollTopDescriptor);
    }
  });

  it('sets the list container scrollTop to the stored value when listScrollTop > 0', () => {
    // Seed the store BEFORE render so useRef(jobs.listScrollTop) captures 350.
    useSessionStore.setState((s) => ({ jobs: { ...s.jobs, listScrollTop: 350 } }));
    renderSplit();
    expect(scrollTopWrites).toContain(350);
  });

  it('does not set scrollTop when listScrollTop is 0 (avoids forced scroll to top)', () => {
    // Default store has listScrollTop: 0 — the guard `> 0` must prevent the write.
    renderSplit();
    expect(scrollTopWrites).toHaveLength(0);
  });
});

// Replace globalThis.requestAnimationFrame/cancelAnimationFrame with a manual
// queue so we control exactly when callbacks fire (deterministic, no real-timer
// dependency). scrollTop is stubbed on the element instance via
// Object.defineProperty because jsdom has no scroll layout.
describe('JobsSplitView — scroll persist (RAF-throttled)', () => {
  // const so the closure in the stub always captures the same array reference.
  const rafQueue: Array<{ id: number; cb: FrameRequestCallback }> = [];
  let rafIdCounter = 0;

  beforeEach(() => {
    rafQueue.splice(0);
    rafIdCounter = 0;
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback): number => {
      const id = ++rafIdCounter;
      rafQueue.push({ id, cb });
      return id;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number): void => {
      const idx = rafQueue.findIndex((r) => r.id === id);
      if (idx >= 0) rafQueue.splice(idx, 1);
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    rafQueue.splice(0);
  });

  /** Run all queued RAF callbacks inside act so Zustand→React updates settle. */
  async function flushRaf(): Promise<void> {
    const toRun = rafQueue.splice(0);
    await act(async () => {
      for (const entry of toRun) {
        entry.cb(0);
      }
    });
  }

  it('writes scrollTop to the store after the RAF fires', async () => {
    renderSplit();
    const listbox = screen.getByRole('listbox');

    // jsdom has no layout — define a readable scrollTop on the element instance.
    const fakeScrollTop = 420;
    Object.defineProperty(listbox, 'scrollTop', { get: () => fakeScrollTop, configurable: true });

    fireEvent.scroll(listbox);
    // RAF is queued but not yet flushed — store must still be at 0.
    expect(useSessionStore.getState().jobs.listScrollTop).toBe(0);

    await flushRaf();

    expect(useSessionStore.getState().jobs.listScrollTop).toBe(420);
  });

  it('throttles: multiple scroll events within one frame produce a single store write', async () => {
    renderSplit();
    const listbox = screen.getByRole('listbox');

    let fakeScrollTop = 100;
    Object.defineProperty(listbox, 'scrollTop', { get: () => fakeScrollTop, configurable: true });

    let writeCount = 0;
    const unsub = useSessionStore.subscribe((state, prev) => {
      if (state.jobs.listScrollTop !== prev.jobs.listScrollTop) writeCount++;
    });

    fireEvent.scroll(listbox); // queues RAF id=1
    fakeScrollTop = 200;
    fireEvent.scroll(listbox); // rafId !== null → skipped by the guard
    fakeScrollTop = 300;
    fireEvent.scroll(listbox); // rafId !== null → skipped by the guard

    await flushRaf();

    unsub();
    // Exactly one RAF callback fired → exactly one store write.
    expect(writeCount).toBe(1);
    // The write uses the scrollTop at flush time (300, the latest value).
    expect(useSessionStore.getState().jobs.listScrollTop).toBe(300);
  });

  it('cancels the pending RAF and removes the scroll listener on unmount', async () => {
    const { unmount } = renderSplit();
    const listbox = screen.getByRole('listbox');

    Object.defineProperty(listbox, 'scrollTop', { get: () => 100, configurable: true });

    fireEvent.scroll(listbox);
    // RAF must be pending before unmount.
    expect(rafQueue).toHaveLength(1);

    // Unmount triggers cleanup: removeEventListener + cancelAnimationFrame.
    unmount();
    expect(rafQueue).toHaveLength(0);

    // Flushing an empty queue must not write to the store.
    await flushRaf();
    expect(useSessionStore.getState().jobs.listScrollTop).toBe(0);
  });
});

// Two defects, one contract. (1) `md:` is a VIEWPORT breakpoint and is always
// active at the 900px window floor, so it forced two panes even when the
// expanded sidebar left the results card ~390px wide — the gate has to read the
// card's own width (`@2xl` = 42rem — the card reaches it at a ~1100px window
// and still yields 280px of list + ~434px of detail; `@3xl` needed 1200px,
// 1340px at large text scale, which starved the narrow windows this redesign
// exists to serve — docs/PATTERNS.md §15). (2) The fixed
// `md:w-[420px] xl: 2xl:` ladder left the detail pane ~248px, which is what
// clipped its action cluster. jsdom has no layout engine, so the contract is
// asserted on the classes that produce it — the same seam ModalShell's
// responsive test uses.
describe('JobsSplitView — responsive width contract', () => {
  function panes(): { root: HTMLElement; aside: HTMLElement; detail: HTMLElement } {
    const aside = screen.getByRole('complementary');
    const root = aside.parentElement;
    const detail = screen.getByTestId('job-detail').closest('section');
    if (!root || !detail) throw new Error('split panes not rendered');
    return { root, aside, detail };
  }

  it('sizes the list pane proportionally with a floor and a ceiling, not a fixed px ladder', () => {
    renderSplit();
    const { aside } = panes();

    expect(aside.className).toContain('@2xl:w-[34%]');
    expect(aside.className).toContain('@2xl:min-w-[280px]');
    expect(aside.className).toContain('@2xl:max-w-[480px]');
    // The fixed ladder is what starved the detail pane at the 900px floor.
    expect(aside.className).not.toContain('w-[420px]');
    expect(aside.className).not.toContain('xl:w-[480px]');
    expect(aside.className).not.toContain('2xl:w-[520px]');
  });

  it('keeps the list pane from being squeezed by detail-pane content', () => {
    renderSplit();
    expect(panes().aside.className).toContain('@2xl:shrink-0');
  });

  it('gates two-pane on the CARD width, never on the viewport', () => {
    renderSplit();
    const { root, aside, detail } = panes();

    // Every layout decision is a container query. A viewport `md:` fired at the
    // 900px floor even when the sidebar left the card far too narrow to split.
    expect(root.className).toContain('@2xl:flex-row');
    expect(root.className).not.toMatch(/(^|\s)md:/);
    expect(aside.className).not.toMatch(/(^|\s)md:/);
    expect(detail.className).not.toMatch(/(^|\s)md:/);
  });

  it('lets the root fill the results card instead of resolving to max-content', () => {
    renderSplit();
    const { root } = panes();
    // Without w-full + min-w-0 this lone flex child sizes to max-content and the
    // two panes overflow the card (which clips with overflow-hidden).
    expect(root.className).toContain('w-full');
    expect(root.className).toContain('min-w-0');
  });

  it('keeps the detail pane shrinkable (min-w-0) so it can compress rather than overflow', () => {
    renderSplit();
    const { detail } = panes();
    expect(detail.className).toContain('min-w-0');
    expect(detail.className).toContain('flex-1');
  });
});
