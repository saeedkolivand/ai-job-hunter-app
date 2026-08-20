/**
 * RootErrorBoundary — the root route's `errorComponent`.
 *
 * Regression: `AutopilotPage` threw a Rules-of-Hooks error (2026-08-18) and
 * the app white-screened — TanStack Router had no `errorComponent` anywhere
 * in the tree to catch it. This only covers the boundary component itself
 * (real i18n copy renders, retry wires to the router's `reset`) — there is no
 * repro for the underlying hook-order bug, so that is out of scope here.
 *
 * Renders `RootErrorBoundary` standalone (not the full root route tree). It
 * depends on `useTranslation`, `ErrorState` and `useNavigate` — the router hook
 * is MOCKED here rather than wrapped in a real router, so the assertion is
 * "the button asks to go to `/`", not "the router moved".
 *
 * Standalone rendering is why this file cannot prove the shell unmounts on a
 * catch (it does — the root match's component IS `RootLayout`). That is why the
 * copy must not mention a sidebar and why the dashboard action exists at all.
 */
import { describe, expect, it, vi } from 'vitest';
import type * as RouterModule from '@tanstack/react-router';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import i18n from '@ajh/translations';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof RouterModule>()),
  useNavigate: () => navigate,
}));

import { RootErrorBoundary } from './__root';

describe('RootErrorBoundary', () => {
  it('renders the real translated title + description (never a raw i18n key)', () => {
    render(<RootErrorBoundary error={new Error('boom')} reset={vi.fn()} />);

    expect(screen.getByText(i18n.t('errorBoundary.title'))).toBeInTheDocument();
    expect(screen.getByText(i18n.t('errorBoundary.description'))).toBeInTheDocument();
  });

  it('the retry action calls the router-supplied reset, not a page reload', async () => {
    const reset = vi.fn();
    const user = userEvent.setup();
    render(<RootErrorBoundary error={new Error('boom')} reset={reset} />);

    // `ErrorState`'s retry button label ("Try again") is a fixed string in
    // the shared primitive itself, not a translation key of this component's.
    await user.click(screen.getByRole('button', { name: 'Try again' }));

    expect(reset).toHaveBeenCalledTimes(1);
  });

  it('offers a dashboard escape hatch, because reset alone loops on a render bug', async () => {
    // The whole shell (Sidebar included) is replaced on a catch, so navigation
    // is the only real way out — and `reset` re-renders the route that threw,
    // which returns straight to this screen for a deterministic crash.
    navigate.mockClear();
    const user = userEvent.setup();
    render(<RootErrorBoundary error={new Error('boom')} reset={vi.fn()} />);

    await user.click(screen.getByRole('button', { name: i18n.t('errorBoundary.goHome') }));

    expect(navigate).toHaveBeenCalledWith({ to: '/' });
  });

  it('renders role="alert" so a screen reader announces the crash', () => {
    render(<RootErrorBoundary error={new Error('boom')} reset={vi.fn()} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});
