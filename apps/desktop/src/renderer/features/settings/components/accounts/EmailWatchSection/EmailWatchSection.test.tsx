import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { EmailWatchStatus } from '@ajh/shared';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { EmailWatchSection } from './index';

const DISCONNECTED: EmailWatchStatus = { connected: false, enabled: false };
const CONNECTED: EmailWatchStatus = {
  connected: true,
  address: 'me@gmail.com',
  enabled: true,
  lastCheckAt: 1_700_000_000_000,
};

function renderSection(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  const client = createMockClient({
    'emailWatch.status': vi.fn().mockResolvedValue(DISCONNECTED),
    ...overrides,
  });
  const queryClient = makeQueryClient();

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <AppClientProvider client={client}>
          <NotificationProvider>{children}</NotificationProvider>
        </AppClientProvider>
      </QueryClientProvider>
    );
  }

  const result = render(<EmailWatchSection />, { wrapper: Wrapper });
  return { ...result, client, queryClient };
}

describe('EmailWatchSection — disconnected → connect', () => {
  it('connects with the typed address + app password, then clears the password field', async () => {
    const connect = vi.fn().mockResolvedValue(CONNECTED);
    renderSection({ 'emailWatch.connect': connect });

    await waitFor(() => screen.getByLabelText('Email address'));

    await userEvent.type(screen.getByLabelText('Email address'), 'me@gmail.com');
    await userEvent.type(screen.getByLabelText('App password'), 'abcd efgh ijkl mnop');

    await userEvent.click(screen.getByRole('button', { name: 'Connect' }));

    await waitFor(() => {
      expect(connect).toHaveBeenCalledWith({
        address: 'me@gmail.com',
        appPassword: 'abcd efgh ijkl mnop',
      });
    });

    // Once connected, the form (and its password field) is no longer rendered at all —
    // the app password never lingers in the DOM after the mutation fires.
    await waitFor(() => {
      expect(screen.queryByLabelText('App password')).not.toBeInTheDocument();
    });
  });

  it('surfaces a connect failure inline (fixed copy, not the raw error)', async () => {
    const connect = vi.fn().mockRejectedValue(new Error('IMAP LOGIN failed'));
    renderSection({ 'emailWatch.connect': connect });

    await waitFor(() => screen.getByLabelText('Email address'));
    await userEvent.type(screen.getByLabelText('Email address'), 'me@gmail.com');
    await userEvent.type(screen.getByLabelText('App password'), 'wrong-app-password');
    await userEvent.click(screen.getByRole('button', { name: 'Connect' }));

    await waitFor(() => {
      expect(
        screen.getByText('Could not connect. Check the address and app password and try again.')
      ).toBeInTheDocument();
    });
    // Never the raw rejection text.
    expect(screen.queryByText('IMAP LOGIN failed')).not.toBeInTheDocument();

    // The app password must not linger in state/the DOM after a FAILED
    // connect either — only a fresh paste should ever populate it again.
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLInputElement>('App password').value).toBe('');
    });
  });

  it('disables Connect until both fields are non-empty', async () => {
    renderSection();

    await waitFor(() => screen.getByLabelText('Email address'));
    expect(screen.getByRole('button', { name: 'Connect' })).toBeDisabled();

    await userEvent.type(screen.getByLabelText('Email address'), 'me@gmail.com');
    expect(screen.getByRole('button', { name: 'Connect' })).toBeDisabled();

    await userEvent.type(screen.getByLabelText('App password'), 'abcd efgh ijkl mnop');
    expect(screen.getByRole('button', { name: 'Connect' })).not.toBeDisabled();
  });
});

describe('EmailWatchSection — connected', () => {
  it('renders the connected address, the enabled toggle, and Check now', async () => {
    renderSection({ 'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED) });

    await waitFor(() => {
      expect(screen.getByText('me@gmail.com')).toBeInTheDocument();
    });

    const sw = screen.getByRole('switch', { name: 'Watch for confirmation emails' });
    expect(sw).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('button', { name: 'Check now' })).toBeInTheDocument();

    // Never a raw i18n key leaking into the DOM.
    const body = document.body.textContent ?? '';
    expect(body).not.toMatch(/settings\.accounts\.emailWatch\./);
  });

  it('calls checkNow when "Check now" is clicked', async () => {
    const checkNow = vi.fn().mockResolvedValue(CONNECTED);
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.checkNow': checkNow,
    });

    const btn = await screen.findByRole('button', { name: 'Check now' });
    await userEvent.click(btn);

    await waitFor(() => {
      expect(checkNow).toHaveBeenCalledTimes(1);
    });
  });

  it('keeps an accessible text label on "Check now" for the whole pending IMAP round trip', async () => {
    let resolveCheck!: (value: EmailWatchStatus) => void;
    const checkNow = vi.fn(
      () => new Promise<EmailWatchStatus>((resolve) => (resolveCheck = resolve))
    );
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.checkNow': checkNow,
    });

    const btn = await screen.findByRole('button', { name: 'Check now' });
    await userEvent.click(btn);

    // Never a bare icon-only button — the pending label stays a real text name.
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Checking…' })).toBeInTheDocument();
    });

    resolveCheck(CONNECTED);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Check now' })).toBeInTheDocument();
    });
  });

  it('keeps an accessible text label on "Disconnect" for the whole pending round trip', async () => {
    let resolveDisconnect!: (value: EmailWatchStatus) => void;
    const disconnect = vi.fn(
      () => new Promise<EmailWatchStatus>((resolve) => (resolveDisconnect = resolve))
    );
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.disconnect': disconnect,
    });

    const btn = await screen.findByRole('button', { name: 'Disconnect' });
    await userEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Disconnecting…' })).toBeInTheDocument();
    });

    resolveDisconnect(DISCONNECTED);
    await waitFor(() => {
      expect(screen.getByLabelText('Email address')).toBeInTheDocument();
    });
  });

  it('returns to the connect form after disconnecting', async () => {
    const disconnect = vi.fn().mockResolvedValue(DISCONNECTED);
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.disconnect': disconnect,
    });

    const disconnectBtn = await screen.findByRole('button', { name: 'Disconnect' });
    await userEvent.click(disconnectBtn);

    await waitFor(() => {
      expect(disconnect).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getByLabelText('Email address')).toBeInTheDocument();
    });
  });

  it('shows a fixed-copy notification when toggling the watch switch fails', async () => {
    const setEnabled = vi.fn().mockRejectedValue(new Error('store write failed'));
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.setEnabled': setEnabled,
    });

    const sw = await screen.findByRole('switch', { name: 'Watch for confirmation emails' });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setEnabled).toHaveBeenCalledWith(false);
    });
    await waitFor(() => {
      expect(screen.getByText('Could not update the email-tracking setting.')).toBeInTheDocument();
    });
  });
});
