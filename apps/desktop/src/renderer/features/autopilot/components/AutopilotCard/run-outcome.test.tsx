/**
 * AutopilotCard — run-outcome badge, per-board chips, board reliability
 *
 * Shared mocks + fixtures live in ./test-render.
 */

import { describe, expect, it } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { makeAutopilot, renderCard, state, withRun } from './test-render';

// ─────────────────────────────────────────────────────────────────────────────
// Persisted run-outcome badge (failed / completedWithErrors / interrupted)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — run-status badge', () => {
  it('renders the failed badge (red) when runStatus is failed', () => {
    renderCard(withRun('failed'));
    expect(screen.getByText('autopilot.badge.failed')).toBeInTheDocument();
  });

  it('renders the partial-results badge when runStatus is completedWithErrors', () => {
    renderCard(withRun('completedWithErrors'));
    expect(screen.getByText('autopilot.badge.completedWithErrors')).toBeInTheDocument();
  });

  it('renders the interrupted badge when runStatus is interrupted', () => {
    renderCard(withRun('interrupted'));
    expect(screen.getByText('autopilot.badge.interrupted')).toBeInTheDocument();
  });

  it('renders NO badge for the happy completed status', () => {
    renderCard(withRun('completed'));
    expect(
      screen.queryByText(/autopilot\.badge\.(failed|completedWithErrors|interrupted)/)
    ).not.toBeInTheDocument();
  });

  it('renders NO badge for an unknown/future status (graceful fallback, never a raw enum)', () => {
    renderCard(withRun('someFutureStatus'));
    expect(
      screen.queryByText(/autopilot\.badge\.(failed|completedWithErrors|interrupted)/)
    ).not.toBeInTheDocument();
    // The raw enum value must never leak into the DOM.
    expect(screen.queryByText('someFutureStatus')).not.toBeInTheDocument();
  });

  it('hides the badge while a run is in progress', () => {
    renderCard(withRun('failed'), { runState: 'scraping' });
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Needs-configuration guard (PR B carry-over 2) + badge hover explainers
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — needs-configuration guard', () => {
  it('a failed run where every board was merely skipped shows a neutral needs-config badge, not red failed', () => {
    renderCard(
      withRun('failed', [
        { board: 'aggregator', count: 0, skipped: 'needs-keys' },
        { board: 'linkedin', count: 0, skipped: 'needs-login' },
      ])
    );
    expect(screen.getByText('autopilot.badge.needsConfig')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
  });

  it('a failed run with a real board error keeps the red failed badge (not needs-config)', () => {
    renderCard(
      withRun('failed', [
        { board: 'linkedin', count: 0, error: '429 Too Many Requests' },
        { board: 'aggregator', count: 0, skipped: 'needs-keys' },
      ])
    );
    expect(screen.getByText('autopilot.badge.failed')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.needsConfig')).not.toBeInTheDocument();
  });

  it('the needs-config badge carries a hover explainer', () => {
    renderCard(withRun('failed', [{ board: 'aggregator', count: 0, skipped: 'needs-keys' }]));
    expect(screen.getByText('autopilot.badge.needsConfigHint')).toBeInTheDocument();
  });

  it('the partial-results badge carries a hover explainer', () => {
    renderCard(withRun('completedWithErrors', [{ board: 'linkedin', count: 0, error: 'boom' }]));
    expect(screen.getByText('autopilot.badge.completedWithErrorsHint')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Persisted per-board chip strip — survives the run ending
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — persisted per-board chips', () => {
  it('renders the last run per-board chips when not running', () => {
    renderCard(
      withRun('completedWithErrors', [
        { board: 'greenhouse', count: 4 },
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    expect(screen.getAllByTestId('chip').length).toBeGreaterThanOrEqual(2);
  });

  it('does NOT render persisted chips while a run is in progress (live log shown instead)', () => {
    renderCard(withRun('completed', [{ board: 'greenhouse', count: 4 }]), {
      runState: 'scraping',
    });
    expect(screen.queryAllByTestId('chip')).toHaveLength(0);
    // Asserted independently of the chips (not just implied by sharing one JSX
    // conditional) so a future refactor decoupling the two is still caught.
    expect(
      screen.queryByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).not.toBeInTheDocument();
  });

  it('shows an info button with a localized aria-label that reveals the chips when persisted summaries exist', () => {
    renderCard(
      withRun('completedWithErrors', [
        { board: 'greenhouse', count: 4 },
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    // The chips themselves stay in the DOM (behind the HoverPopover mock, which
    // renders trigger + content unconditionally) — the meaningful assertion is
    // that the on-demand trigger exists with a real, localized accessible name.
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toBeInTheDocument();
    expect(screen.getAllByTestId('chip').length).toBeGreaterThanOrEqual(2);
  });

  it('does NOT render the info button when there are no persisted summaries', () => {
    renderCard(makeAutopilot());
    expect(
      screen.queryByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).not.toBeInTheDocument();
  });

  it('escalates the info trigger to the degraded tone when a board is merely skipped beside a succeeding one, even though no colored badge fires', () => {
    // Plain `completed` + one skipped board: `RUN_STATUS_BADGE` has no entry
    // for `completed`, so no colored badge renders at all — the info
    // trigger's own tone is the ONLY surviving "something's off" signal.
    renderCard(
      withRun('completed', [
        { board: 'xing', count: 0, skipped: 'needs-login' },
        { board: 'linkedin', count: 5 },
      ])
    );
    expect(screen.queryByText('autopilot.badge.failed')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.completedWithErrors')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.badge.needsConfig')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'true');
  });

  it('keeps the resting (non-degraded) tone when every board succeeded', () => {
    renderCard(withRun('completed', [{ board: 'linkedin', count: 5 }]));
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'false');
  });

  it('does NOT escalate for an informational location note alone (no cry-wolf amber)', () => {
    renderCard(withRun('completed', [{ board: 'linkedin', count: 5, notes: ['broadened:de'] }]));
    expect(
      screen.getByRole('button', { name: 'autopilot.boardResults.infoLabel' })
    ).toHaveAttribute('data-degraded', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Track B1 — board reliability is read LIVE, not off the stored record
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — board reliability', () => {
  it('badges a board using the CURRENT verdict, not one frozen into the record', async () => {
    const user = userEvent.setup();
    // The stored run summary carries NO health (it is stripped on persist) —
    // everything the badge shows must come from the live query.
    state.boardHealth = new Map<string, unknown>([
      [
        'wwr',
        {
          status: 'failing',
          consecutiveFailures: 4,
          verifiedRuns: 9,
          failedRuns: 4,
          lastSuccessAt: Date.now() - 6 * 24 * 60 * 60 * 1000,
          failingSince: Date.now() - 5 * 24 * 60 * 60 * 1000,
        },
      ],
    ]);
    renderCard(withRun('completedWithErrors', [{ board: 'wwr', count: 0, error: 'HTTP 500' }]));

    await user.click(screen.getByLabelText('autopilot.boardResults.infoLabel'));
    expect(await screen.findByText(/health\.failingSince/)).toBeInTheDocument();
  });

  it('shows no reliability badge while the live verdict says the board is fine', async () => {
    const user = userEvent.setup();
    state.boardHealth = new Map<string, unknown>([
      ['wwr', { status: 'healthy', consecutiveFailures: 0, verifiedRuns: 9, failedRuns: 0 }],
    ]);
    renderCard(withRun('completedWithErrors', [{ board: 'wwr', count: 0, error: 'HTTP 500' }]));

    await user.click(screen.getByLabelText('autopilot.boardResults.infoLabel'));
    // This run's own failure is still explained…
    expect(await screen.findByText(/HTTP 500/)).toBeInTheDocument();
    // …but nothing claims a standing outage.
    expect(screen.queryByText(/health\./)).toBeNull();
  });
});
