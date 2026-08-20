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

// autoWriteEnabled defaults to false in both fixtures — the backend's real
// default after five security rounds (the sender-authentication gate is
// best-effort and can be fooled, so nobody gets auto-write silently). Tests
// that specifically exercise the ON state override it explicitly.
const DISCONNECTED: EmailWatchStatus = {
  connected: false,
  enabled: false,
  autoWriteEnabled: false,
};
const CONNECTED: EmailWatchStatus = {
  connected: true,
  address: 'me@gmail.com',
  enabled: true,
  lastCheckAt: 1_700_000_000_000,
  autoWriteEnabled: false,
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

  it('shows generic checkNowFailed copy for an ordinary checkNow failure', async () => {
    const checkNow = vi.fn().mockRejectedValue(new Error('IMAP fetch failed'));
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.checkNow': checkNow,
    });

    const btn = await screen.findByRole('button', { name: 'Check now' });
    await userEvent.click(btn);

    await waitFor(() => {
      expect(
        screen.getByText('Could not reach the mailbox. Check your app password and try again.')
      ).toBeInTheDocument();
    });
  });

  it('shows friendly rate-limit copy (not the raw backend sentinel) when checkNow is rate-limited', async () => {
    const checkNow = vi
      .fn()
      .mockRejectedValue(new Error('a check already ran recently — try again in a moment'));
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.checkNow': checkNow,
    });

    const btn = await screen.findByRole('button', { name: 'Check now' });
    await userEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByText('Checked moments ago — try again in a minute.')).toBeInTheDocument();
    });
    expect(
      screen.queryByText('a check already ran recently — try again in a moment')
    ).not.toBeInTheDocument();
  });

  it('also recognizes the rate-limit sentinel when Tauri rejects with a bare string (not an Error)', async () => {
    const checkNow = vi
      .fn()
      .mockRejectedValue('a check already ran recently — try again in a moment');
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.checkNow': checkNow,
    });

    const btn = await screen.findByRole('button', { name: 'Check now' });
    await userEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByText('Checked moments ago — try again in a minute.')).toBeInTheDocument();
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

describe('EmailWatchSection — auto-write toggle', () => {
  // Pins the new default: OFF unless the user explicitly turns it on. Uses
  // CONNECTED as-is (autoWriteEnabled: false) rather than an override, so a
  // careless flip of the fixture back to `true` would also fail this test.
  it('reads OFF by default — reflects autoWriteEnabled: false from the backend', async () => {
    renderSection({ 'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED) });

    const sw = await screen.findByRole('switch', { name: 'Update status automatically' });
    expect(sw).toHaveAttribute('aria-checked', 'false');
  });

  it('reflects autoWriteEnabled: true when the backend says the user turned it on', async () => {
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue({ ...CONNECTED, autoWriteEnabled: true }),
    });

    const sw = await screen.findByRole('switch', { name: 'Update status automatically' });
    expect(sw).toHaveAttribute('aria-checked', 'true');
  });

  it('calls the auto-write setter with the new value when toggled', async () => {
    // Starts OFF (CONNECTED's default) — toggling it turns it ON.
    const setAutoWriteEnabled = vi.fn().mockResolvedValue({ ...CONNECTED, autoWriteEnabled: true });
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.setAutoWriteEnabled': setAutoWriteEnabled,
    });

    const sw = await screen.findByRole('switch', { name: 'Update status automatically' });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAutoWriteEnabled).toHaveBeenCalledWith(true);
    });
  });

  it('shows a fixed-copy notification when toggling auto-write fails', async () => {
    const setAutoWriteEnabled = vi.fn().mockRejectedValue(new Error('store write failed'));
    renderSection({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
      'emailWatch.setAutoWriteEnabled': setAutoWriteEnabled,
    });

    const sw = await screen.findByRole('switch', { name: 'Update status automatically' });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(screen.getByText('Could not update the email-tracking setting.')).toBeInTheDocument();
    });
  });

  // The real backend default is now OFF (was ON before five security
  // rounds). Before the status query resolves, the component must never
  // commit the switch to ON — that is the unsafe direction now: it would
  // suggest auto-write is already protecting/active before that is known.
  // There is exactly ONE gate that gives the switch a value at all —
  // `{status?.connected ? … }` in the component, which is what narrows
  // `status` for the `<Switch checked={status.autoWriteEnabled}>` a few
  // lines below it. So exercising that gate's pending window (this test)
  // is the whole guarantee: resolving to `true` here proves the switch
  // never flashed `true` — or any value — before this point.
  it('never renders the auto-write switch as ON before the status is known — no premature commit', async () => {
    let resolveStatus!: (value: EmailWatchStatus) => void;
    const statusFn = vi.fn(
      () => new Promise<EmailWatchStatus>((resolve) => (resolveStatus = resolve))
    );
    renderSection({ 'emailWatch.status': statusFn });

    // While the query is in flight there is no `connected` value yet either,
    // so the switch (like the rest of the connected view) is simply absent —
    // never present with a committed `aria-checked="true"`.
    expect(
      screen.queryByRole('switch', { name: 'Update status automatically' })
    ).not.toBeInTheDocument();

    resolveStatus({ ...CONNECTED, autoWriteEnabled: true });

    await waitFor(() => {
      expect(screen.getByRole('switch', { name: 'Update status automatically' })).toHaveAttribute(
        'aria-checked',
        'true'
      );
    });
  });
});
