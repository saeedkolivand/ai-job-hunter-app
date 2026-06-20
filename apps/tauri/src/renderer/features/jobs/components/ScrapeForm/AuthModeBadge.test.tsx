/**
 * AuthModeBadge — 3-tier auth badge rendering tests.
 *
 * Covers:
 *   - auth='guest'    → renders nothing (show=false gate)
 *   - auth='optional' + not connected → "Guest mode" amber badge (jobs.modeGuest)
 *   - auth='optional' + connected     → "Authenticated" green badge (jobs.modeAuthenticated)
 *   - auth='required' + not connected → "Login required" red badge + "Log in" button
 *   - auth='required' + not connected + connectPending → spinner instead of "Log in"
 *   - auth='required' + connected     → "Authenticated" green badge (no Log in button)
 *   - onConnect called when "Log in" is clicked
 *   - connected state: disconnect button rendered + onDisconnect called
 *
 * motion/react is globally shimmed in vitest.setup.ts — no per-test mock needed.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AuthModeBadge } from './AuthModeBadge';

// i18n: identity t() so we assert on translation keys (no i18next instance needed)
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type BadgeProps = Partial<React.ComponentProps<typeof AuthModeBadge>>;

function renderBadge({
  show = true,
  board = 'test-board',
  auth = 'optional',
  boardConnected = false,
  disconnectPending = false,
  connectPending = false,
  onDisconnect = vi.fn(),
  onConnect = vi.fn(),
}: BadgeProps = {}) {
  return render(
    <AuthModeBadge
      show={show}
      board={board}
      auth={auth}
      boardConnected={boardConnected}
      disconnectPending={disconnectPending}
      connectPending={connectPending}
      onDisconnect={onDisconnect}
      onConnect={onConnect}
    />
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AuthModeBadge — guest (show=false)', () => {
  it('renders nothing when show=false', () => {
    const { container } = renderBadge({ show: false, auth: 'guest' });
    // AnimatePresence renders nothing when show=false
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when auth=guest and show=false', () => {
    const { container } = renderBadge({ show: false, auth: 'guest', boardConnected: false });
    expect(container.firstChild).toBeNull();
  });
});

describe('AuthModeBadge — optional (LinkedIn) + not connected', () => {
  it('shows the Guest mode amber badge', () => {
    renderBadge({ auth: 'optional', boardConnected: false });
    expect(screen.getByText('jobs.modeGuest')).toBeInTheDocument();
  });

  it('does not show "Login required" text', () => {
    renderBadge({ auth: 'optional', boardConnected: false });
    expect(screen.queryByText('jobs.modeLoginRequired')).not.toBeInTheDocument();
  });

  it('does not render a Log in button', () => {
    renderBadge({ auth: 'optional', boardConnected: false });
    expect(screen.queryByText('jobs.logIn')).not.toBeInTheDocument();
  });
});

describe('AuthModeBadge — optional + connected', () => {
  it('shows the Authenticated badge when connected', () => {
    renderBadge({ auth: 'optional', boardConnected: true });
    expect(screen.getByText('jobs.modeAuthenticated')).toBeInTheDocument();
  });

  it('does not show the guest mode badge when connected', () => {
    renderBadge({ auth: 'optional', boardConnected: true });
    expect(screen.queryByText('jobs.modeGuest')).not.toBeInTheDocument();
  });

  it('shows the Disconnect button when connected', () => {
    renderBadge({ auth: 'optional', boardConnected: true });
    expect(screen.getByText('jobs.disconnect')).toBeInTheDocument();
  });

  it('calls onDisconnect when the disconnect button is clicked', async () => {
    const onDisconnect = vi.fn();
    renderBadge({ auth: 'optional', boardConnected: true, onDisconnect });
    await userEvent.click(screen.getByText('jobs.disconnect'));
    expect(onDisconnect).toHaveBeenCalledTimes(1);
  });
});

describe('AuthModeBadge — required (indeed / xing) + not connected', () => {
  it('shows the Login required red badge', () => {
    renderBadge({ auth: 'required', boardConnected: false });
    expect(screen.getByText('jobs.modeLoginRequired')).toBeInTheDocument();
  });

  it('shows the login-required note', () => {
    renderBadge({ auth: 'required', boardConnected: false });
    expect(screen.getByText('jobs.modeLoginRequiredNote')).toBeInTheDocument();
  });

  it('renders the inline Log in button', () => {
    renderBadge({ auth: 'required', boardConnected: false });
    expect(screen.getByText('jobs.logIn')).toBeInTheDocument();
  });

  it('calls onConnect when the Log in button is clicked', async () => {
    const onConnect = vi.fn();
    renderBadge({ auth: 'required', boardConnected: false, onConnect });
    await userEvent.click(screen.getByText('jobs.logIn'));
    expect(onConnect).toHaveBeenCalledTimes(1);
  });

  it('disables the Log in button and shows a spinner when connectPending=true', () => {
    renderBadge({ auth: 'required', boardConnected: false, connectPending: true });
    // The visible label is replaced by a spinner; the text is now sr-only (visually hidden)
    const srLabel = screen.getByText('jobs.logIn');
    expect(srLabel).toHaveClass('sr-only');
    // The button remains accessible via aria-label and is disabled
    const connectBtn = screen.getByRole('button', { name: 'jobs.logIn' });
    expect(connectBtn).toBeDisabled();
  });

  it('does not show the Guest mode badge for required boards', () => {
    renderBadge({ auth: 'required', boardConnected: false });
    expect(screen.queryByText('jobs.modeGuest')).not.toBeInTheDocument();
  });
});

describe('AuthModeBadge — required + connected', () => {
  it('shows the Authenticated badge when a required board is connected', () => {
    renderBadge({ auth: 'required', boardConnected: true });
    expect(screen.getByText('jobs.modeAuthenticated')).toBeInTheDocument();
  });

  it('does not show the Login required badge when connected', () => {
    renderBadge({ auth: 'required', boardConnected: true });
    expect(screen.queryByText('jobs.modeLoginRequired')).not.toBeInTheDocument();
  });

  it('does not render the Log in button when connected', () => {
    renderBadge({ auth: 'required', boardConnected: true });
    expect(screen.queryByText('jobs.logIn')).not.toBeInTheDocument();
  });
});
