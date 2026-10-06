/**
 * AutopilotCard — header toggle / keyboard, per-card sort, focusedJobUrl scroll + highlight
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AutopilotFoundJob } from '@ajh/shared';

import {
  AutopilotCard,
  cardHeader,
  defaultProps,
  expandCard,
  makeAutopilot,
  makeJob,
  renderCard,
} from './test-render';

// ─────────────────────────────────────────────────────────────────────────────
// handleHeaderToggle
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleHeaderToggle', () => {
  it('clicking the header toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    // Found jobs panel is initially hidden.
    expect(screen.queryByText('autopilot.foundJobs · 1')).not.toBeInTheDocument();

    // The header div carries aria-expanded — use that as the unique selector.
    const headerDiv = document.querySelector('[aria-expanded]') as HTMLElement;
    expect(headerDiv).not.toBeNull();
    await user.click(headerDiv);

    // Panel is now visible: the inner heading contains the count.
    expect(screen.getByText(/autopilot\.foundJobs · 1/)).toBeInTheDocument();

    // Click again to collapse.
    await user.click(headerDiv);
    expect(screen.queryByText(/autopilot\.foundJobs · 1/)).not.toBeInTheDocument();
  });

  it('does NOT toggle when foundJobs is empty (no role=button on header)', () => {
    renderCard(makeAutopilot([]));
    // No header button role exists when there are no found jobs.
    expect(
      screen.queryByRole('button', { name: /autopilot.foundJobs: My Autopilot/i })
    ).not.toBeInTheDocument();
  });

  it('header carries aria-expanded=false initially when foundJobs present', () => {
    renderCard(makeAutopilot([makeJob()]));
    const header = document.querySelector('[aria-expanded]');
    expect(header).not.toBeNull();
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('aria-expanded becomes true after toggle', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    await user.click(cardHeader());
    expect(cardHeader()).toHaveAttribute('aria-expanded', 'true');
  });

  it('aria-label switches between foundJobs and collapse on toggle', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    // Initially: "autopilot.foundJobs: My Autopilot"
    expect(header).toHaveAttribute('aria-label', 'autopilot.foundJobs: My Autopilot');

    await user.click(header);
    // After expand: "autopilot.collapse: My Autopilot"
    expect(header).toHaveAttribute('aria-label', 'autopilot.collapse: My Autopilot');
  });

  it('clicking the actions cluster (stopPropagation) does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    expect(header).toHaveAttribute('aria-expanded', 'false');

    // The Run button is inside the actions cluster which stopPropagation.
    // After clicking Run, aria-expanded should still be false.
    const runButton = screen.getByRole('button', { name: /autopilot\.wizard\.run/i });
    await user.click(runButton);

    expect(header).toHaveAttribute('aria-expanded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleHeaderKeyDown
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleHeaderKeyDown', () => {
  it('Enter key toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{Enter}');

    expect(header).toHaveAttribute('aria-expanded', 'true');
  });

  it('Space key toggles showFound when foundJobs.length > 0', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard(' ');

    expect(header).toHaveAttribute('aria-expanded', 'true');
  });

  it('Enter then Enter collapses again', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{Enter}');
    expect(header).toHaveAttribute('aria-expanded', 'true');

    await user.keyboard('{Enter}');
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('Tab key does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    // Tab moves focus — no toggle expected.
    await user.keyboard('{Tab}');
    // aria-expanded remains false regardless of where focus went.
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });

  it('ArrowDown key does NOT toggle showFound', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot([makeJob()]));

    const header = document.querySelector('[aria-expanded]') as HTMLElement;
    header.focus();
    await user.keyboard('{ArrowDown}');
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// AutopilotCard — foundJobs honors its OWN local per-card sort state (owner
// correction: sort is per-autopilot, not a shared session-store field — two
// expanded cards must be sortable independently). The Dropdown lives in the
// found-jobs panel header; option buttons are the mocked `@ajh/ui` Dropdown
// above.
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — foundJobs honors its own per-card sort', () => {
  /** DOM render order via the row's existing `data-job-url` seam. */
  function domJobOrder(): (string | null)[] {
    return Array.from(document.querySelectorAll('[data-job-url]')).map((el) =>
      el.getAttribute('data-job-url')
    );
  }

  function mixedJobs(): AutopilotFoundJob[] {
    return [
      { ...makeJob('https://example.com/c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/a'), postedAt: 3_000 },
      makeJob('https://example.com/b'), // undated
    ];
  }

  /** Expand the panel, then click the per-card Dropdown's "Newest" option. */
  async function expandAndSelectNewest(user: ReturnType<typeof userEvent.setup>) {
    await user.click(cardHeader());
    await user.click(screen.getByRole('button', { name: 'jobs.sortNewest' }));
  }

  it("defaults to relevance (today's stored rank order) — no reordering", async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot(mixedJobs()));
    await user.click(cardHeader());

    expect(domJobOrder()).toEqual([
      'https://example.com/c',
      'https://example.com/a',
      'https://example.com/b',
    ]);
  });

  it('reorders newest-first (dated band leads, undated trails) once the user selects "Newest"', async () => {
    const user = userEvent.setup();
    renderCard(makeAutopilot(mixedJobs()));
    await expandAndSelectNewest(user);

    expect(domJobOrder()).toEqual([
      'https://example.com/a', // postedAt 3000 — newest
      'https://example.com/c', // postedAt 1000
      'https://example.com/b', // undated — trailing band
    ]);
  });

  it('does NOT mutate ap.foundJobs after selecting "Newest" (ADR-020)', async () => {
    const user = userEvent.setup();
    const fixture = mixedJobs();
    const originalOrder = fixture.map((j) => j.url);

    renderCard(makeAutopilot(fixture));
    await expandAndSelectNewest(user);

    expect(fixture.map((j) => j.url)).toEqual(originalOrder);
  });

  // The acceptance bar for the per-card requirement: two cards rendered at
  // once must sort independently — selecting "Newest" on one must NOT affect
  // the other, which stays at its own default (relevance).
  it('two cards sort independently — card A "newest" leaves card B at "relevance"', async () => {
    const user = userEvent.setup();
    const jobsA: AutopilotFoundJob[] = [
      { ...makeJob('https://example.com/a-c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/a-a'), postedAt: 3_000 },
    ];
    const jobsB: AutopilotFoundJob[] = [
      { ...makeJob('https://example.com/b-c'), postedAt: 1_000 },
      { ...makeJob('https://example.com/b-a'), postedAt: 3_000 },
    ];
    const autopilotA = makeAutopilot(jobsA);
    const autopilotB = { ...makeAutopilot(jobsB), _id: 'ap-2', name: 'Second Autopilot' };

    const { container } = render(
      <>
        <AutopilotCard autopilot={autopilotA} {...defaultProps} />
        <AutopilotCard autopilot={autopilotB} {...defaultProps} />
      </>
    );

    const headers = Array.from(container.querySelectorAll('[aria-expanded]'));
    expect(headers).toHaveLength(2);
    const [headerA, headerB] = headers;
    if (!headerA || !headerB) throw new Error('both card headers must be present');
    await user.click(headerA);
    await user.click(headerB);

    // Both cards' Dropdowns share the same accessible group name ("jobs.sort")
    // — scope to the FIRST one (card A, DOM/render order) so only its sort
    // changes.
    const groups = screen.getAllByRole('group', { name: 'jobs.sort' });
    expect(groups).toHaveLength(2);
    const [groupA] = groups;
    if (!groupA) throw new Error('card A sort group must be present');
    await user.click(within(groupA).getByRole('button', { name: 'jobs.sortNewest' }));

    const orderFor = (prefix: string) =>
      domJobOrder().filter((url) => url?.includes(prefix)) as string[];

    // Card A: reordered newest-first.
    expect(orderFor('/a-')).toEqual(['https://example.com/a-a', 'https://example.com/a-c']);
    // Card B: untouched — still its own stored (relevance) order.
    expect(orderFor('/b-')).toEqual(['https://example.com/b-c', 'https://example.com/b-a']);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// focusedJobUrl — scroll-to-row + transient highlight (Back-navigation fix)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — focusedJobUrl scroll + highlight', () => {
  let scrollSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    scrollSpy = vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(() => {});
  });

  afterEach(() => {
    scrollSpy.mockRestore();
    vi.useRealTimers();
  });

  it('scrolls the row matching focusedJobUrl into view, not the header', () => {
    const jobUrl = 'https://example.com/job/42';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).not.toBeNull();
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(row);
    expect(scrollSpy).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: 'smooth', block: 'center' })
    );
  });

  it('applies the transient highlight ring to the targeted row', () => {
    const jobUrl = 'https://example.com/job/highlight';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).toHaveClass('ring-brand/60');
  });

  it('fades the highlight after ~1.5s', () => {
    vi.useFakeTimers();
    const jobUrl = 'https://example.com/job/fade';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: jobUrl });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(row).toHaveClass('ring-brand/60');

    act(() => {
      vi.advanceTimersByTime(1500);
    });

    expect(row).not.toHaveClass('ring-brand/60');
  });

  it('calls onFocusHandled once the row has been scrolled to', () => {
    const jobUrl = 'https://example.com/job/handled';
    const onFocusHandled = vi.fn();
    renderCard(makeAutopilot([makeJob(jobUrl)]), {
      focused: true,
      focusedJobUrl: jobUrl,
      onFocusHandled,
    });

    expect(onFocusHandled).toHaveBeenCalledTimes(1);
  });

  it('falls back to centering the header when focusedJobUrl is absent', () => {
    const jobUrl = 'https://example.com/job/no-focus-url';
    renderCard(makeAutopilot([makeJob(jobUrl)]), { focused: true, focusedJobUrl: null });

    const header = document.querySelector('[aria-expanded]');
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(header);
  });

  it('scrolls via the rAF fallback when the panel is already expanded (no enter animation fires)', async () => {
    // Sync rAF stub — jsdom's real rAF is timer-based; this makes the fallback
    // resolve synchronously within the test's act() calls.
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0);
        return 0;
      });
    const onFocusHandled = vi.fn();
    const jobUrl = 'https://example.com/job/already-expanded';
    const autopilot = makeAutopilot([makeJob(jobUrl)]);
    const { rerender } = renderCard(autopilot, { focused: false });

    // Manually expand via the header — NOT via `focused` — so the found-jobs
    // panel's enter animation (and its onAnimationComplete) has already fired
    // and settled before focus arrives.
    await expandCard();
    expect(cardHeader()).toHaveAttribute('aria-expanded', 'true');
    expect(scrollSpy).not.toHaveBeenCalled();

    // Focus now arrives while already expanded: `setShowFound(true)` is a
    // no-op, so onAnimationComplete never re-fires — only the rAF fallback
    // can resolve the pending scroll.
    await act(async () => {
      rerender(
        <AutopilotCard
          autopilot={autopilot}
          {...defaultProps}
          focused
          focusedJobUrl={jobUrl}
          onFocusHandled={onFocusHandled}
        />
      );
    });

    const row = document.querySelector(`[data-job-url="${jobUrl}"]`);
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    const instance = scrollSpy.mock.instances[0];
    if (!instance) throw new Error('scrollIntoView was not called');
    expect(instance).toBe(row);
    expect(onFocusHandled).toHaveBeenCalledTimes(1);

    rafSpy.mockRestore();
  });
});
