/**
 * SpotlightTour — focused render + navigation tests.
 *
 * Strategy:
 *  - Renders the REAL SpotlightTour (no stub). The wizard tests stub it out;
 *    this file is the dedicated coverage for the component itself.
 *  - @ajh/translations → key-passthrough t() so assertions target i18n key
 *    strings, not actual translations (confirmed pattern from existing tests).
 *  - @ajh/ui → real module spread so Button / cn / transition resolve correctly.
 *  - withProviders + createMockClient supply the React Query / AppClient tree.
 *  - TOUR_ITEMS has 8 entries. The applications item sits at index 1 (inserted
 *    after dashboard). Tests assert both the ordering and the total count.
 *
 * noUncheckedIndexedAccess: all array accesses are guarded.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string, _opts?: unknown) => k }),
}));

// ── @ajh/ui: spread real module so Button / motion helpers resolve ─────────────

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return { ...(actual as object) };
});

// ── component under test (imported AFTER mocks) ───────────────────────────────

import { SpotlightTour } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderTour(onFinish = vi.fn()) {
  const client = createMockClient();
  const result = render(<SpotlightTour onFinish={onFinish} />, {
    wrapper: withProviders(client),
  });
  return { ...result, onFinish };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('SpotlightTour — initial render', () => {
  it('shows the dashboard card on mount (first item, index 0)', () => {
    renderTour();
    // t() is key-passthrough: titleKey → 'onboarding.tour.dashboard.title'
    expect(screen.getByText('onboarding.tour.dashboard.title')).toBeInTheDocument();
    expect(screen.getByText('onboarding.tour.dashboard.desc')).toBeInTheDocument();
  });

  it('first-step Next button label is the "next" key (not "finish")', () => {
    renderTour();
    expect(screen.getByRole('button', { name: 'onboarding.tour.next' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'onboarding.tour.finish' })
    ).not.toBeInTheDocument();
  });
});

describe('SpotlightTour — index-1 ordering (applications)', () => {
  it('advancing one step from dashboard shows the applications card', async () => {
    const user = userEvent.setup();
    renderTour();

    // Confirm we start on dashboard
    expect(screen.getByText('onboarding.tour.dashboard.title')).toBeInTheDocument();

    // Advance one step via the Next button (key-passthrough label)
    await user.click(screen.getByRole('button', { name: 'onboarding.tour.next' }));

    // applications must be at index 1 — directly after dashboard
    expect(screen.getByText('onboarding.tour.applications.title')).toBeInTheDocument();
    expect(screen.getByText('onboarding.tour.applications.desc')).toBeInTheDocument();

    // dashboard card must no longer be visible
    expect(screen.queryByText('onboarding.tour.dashboard.title')).not.toBeInTheDocument();
  });
});

describe('SpotlightTour — item count === 8', () => {
  it('has exactly 8 step-dot indicators', () => {
    const { container } = renderTour();
    // Dots are now rendered by the @ajh/ui StepDots component (non-interactive
    // <div>s, not buttons). Each dot div carries `h-1 rounded-full ...`; the
    // StepDots wrapper div does not (it is a `flex` row). Scope to the dot divs.
    const dots = container.querySelectorAll('div.h-1.rounded-full');
    expect(dots).toHaveLength(8);
  });

  it('clicking Next 7 times reaches the last item and shows the finish button', async () => {
    const user = userEvent.setup();
    renderTour();

    // 8 items → 7 Next clicks to advance from index 0 to index 7
    for (let i = 0; i < 7; i++) {
      const nextBtn = screen.queryByRole('button', { name: 'onboarding.tour.next' });
      if (!nextBtn) throw new Error(`expected Next button before advance ${i}`);
      await user.click(nextBtn);
    }

    // After 7 advances we are on item 7 (last); button must now read "finish"
    expect(screen.getByRole('button', { name: 'onboarding.tour.finish' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'onboarding.tour.next' })).not.toBeInTheDocument();
  });
});

describe('SpotlightTour — onFinish callback', () => {
  it('calls onFinish exactly once when the finish button is clicked on the last item', async () => {
    const user = userEvent.setup();
    const onFinish = vi.fn();
    renderTour(onFinish);

    // Advance to the last item
    for (let i = 0; i < 7; i++) {
      const nextBtn = screen.queryByRole('button', { name: 'onboarding.tour.next' });
      if (!nextBtn) throw new Error(`expected Next button before advance ${i}`);
      await user.click(nextBtn);
    }

    const finishBtn = screen.getByRole('button', { name: 'onboarding.tour.finish' });
    await user.click(finishBtn);

    expect(onFinish).toHaveBeenCalledTimes(1);
  });

  it('calls onFinish immediately when the skip button is clicked on any step', async () => {
    const user = userEvent.setup();
    const onFinish = vi.fn();
    renderTour(onFinish);

    // Skip from the very first step — should call onFinish without advancing
    await user.click(screen.getByRole('button', { name: 'onboarding.tour.skip' }));

    expect(onFinish).toHaveBeenCalledTimes(1);
  });
});
