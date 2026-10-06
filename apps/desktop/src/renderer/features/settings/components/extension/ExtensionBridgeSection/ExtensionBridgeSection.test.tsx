import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ExtensionBridgeStatus } from '@ajh/shared';

import { installBridgeHooks, renderSection, renderWithClient } from './test-support';

vi.mock('@/services', async (importOriginal) =>
  (await import('./services-stub')).servicesMock(importOriginal)
);

installBridgeHooks();

// Pairing token, status pill, clipboard, refresh and the regenerate-confirm flow.
// Autofill + AI answer-assist toggles live in `autofill-and-ai-assist.test.tsx`.
describe('ExtensionBridgeSection', () => {
  it('displays the pairing token value from the hook', async () => {
    renderSection({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' });

    // The token is rendered in a read-only Input — query by its value attribute.
    await waitFor(() => {
      const input = screen.getByRole<HTMLInputElement>('textbox');
      expect(input.value).toBe('tok-abc123');
    });
  });

  it('displays the port number from the hook', async () => {
    renderSection({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByText('9712')).toBeInTheDocument();
    });
  });

  it('shows the connected pill when status.connected is true', async () => {
    renderSection({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' });

    // The translated "Connected" label (not the raw key).
    await waitFor(() => {
      expect(screen.getByText('Connected')).toBeInTheDocument();
    });
  });

  it('shows the disconnected pill when status.connected is false', async () => {
    renderSection({ port: 9712, connected: false, lastSeenMs: null, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByText('Not connected')).toBeInTheDocument();
    });
  });

  // #1258: an MV3 service worker is evicted when idle and drops its socket, so
  // `connected: false` is the NORMAL state for a healthy pairing. Reporting
  // "Not connected" there reads as a fault and pushes the user toward
  // Regenerate, which is exactly the wrong move.
  it('shows paired — not disconnected — when the socket is idle but seen recently', async () => {
    renderSection({
      port: 9712,
      connected: false,
      lastSeenMs: Date.now() - 60_000,
      token: 'tok-abc123',
    });

    await waitFor(() => {
      expect(screen.getByText('Paired')).toBeInTheDocument();
    });
    expect(screen.queryByText('Not connected')).not.toBeInTheDocument();
  });

  it('still shows disconnected when the extension has never paired', async () => {
    renderSection({ port: 9712, connected: false, lastSeenMs: null, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByText('Not connected')).toBeInTheDocument();
    });
    expect(screen.queryByText('Paired')).not.toBeInTheDocument();
  });

  it('renders translated labels — not raw i18n key strings', async () => {
    renderSection();

    await waitFor(() => {
      // Section title is the translated value, not the namespace.key form.
      expect(screen.getByText('Browser extension')).toBeInTheDocument();
    });

    // None of the visible text should be a raw key path.
    const body = document.body.textContent ?? '';
    expect(body).not.toMatch(/settings\.accounts\.extension\./);
  });

  it('calls navigator.clipboard.writeText with the token when Copy is clicked', async () => {
    renderSection({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' });

    await waitFor(() => screen.getByRole('button', { name: /copy/i }));
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('tok-abc123');
  });

  it('does not call clipboard.writeText when the token is empty', async () => {
    renderSection({ port: null, connected: false, lastSeenMs: null, token: '' });

    await waitFor(() => screen.getByRole('button', { name: /copy/i }));
    // The Copy button is disabled when token is empty — click should be a no-op.
    const btn = screen.getByRole('button', { name: /copy/i });
    expect(btn).toBeDisabled();

    // Even if somehow triggered, clipboard must not be called.
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it('clicking the refresh button re-fetches the status query and keeps its label visible while pending', async () => {
    // The second call (the manual refetch triggered by the click) stays
    // pending until the test resolves it, so we can assert the label + spin
    // state mid-flight.
    let resolvePending: (v: ExtensionBridgeStatus) => void = () => {};
    const statusFn = vi
      .fn()
      .mockResolvedValueOnce({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' })
      .mockImplementationOnce(
        () =>
          new Promise<ExtensionBridgeStatus>((resolve) => {
            resolvePending = resolve;
          })
      );
    renderWithClient({ 'extensionBridge.status': statusFn });

    const refreshBtn = await screen.findByRole('button', { name: /refresh/i });
    await waitFor(() => expect(statusFn).toHaveBeenCalledTimes(1));

    await userEvent.click(refreshBtn);

    // The button's accessible label must never vanish while pending — only
    // its icon animates.
    expect(screen.getByRole('button', { name: /refresh/i })).toBeInTheDocument();
    expect(statusFn).toHaveBeenCalledTimes(2);
    expect(refreshBtn.querySelector('svg')).toHaveClass('animate-spin');

    resolvePending({ port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /refresh/i })).toBeInTheDocument();
      expect(refreshBtn.querySelector('svg')).not.toHaveClass('animate-spin');
    });
  });

  it('opens the ConfirmModal when Regenerate is clicked (modal gates mutation)', async () => {
    renderSection();

    await waitFor(() => screen.getByRole('button', { name: /regenerate token/i }));
    await userEvent.click(screen.getByRole('button', { name: /regenerate token/i }));

    // The confirm dialog should now be visible.
    await waitFor(() => {
      expect(screen.getByText('Regenerate pairing token')).toBeInTheDocument();
    });
  });

  it('calls regenerateToken mutation only after the confirm button is clicked', async () => {
    const regenerateToken = vi.fn().mockResolvedValue({ token: 'tok-new' });
    renderWithClient({
      'extensionBridge.status': vi.fn().mockResolvedValue({
        port: 9712,
        connected: true,
        lastSeenMs: null,
        token: 'tok-abc123',
      }),
      'extensionBridge.regenerateToken': regenerateToken,
    });

    await waitFor(() => screen.getByRole('button', { name: /regenerate token/i }));

    // Before opening modal: mutation must not have been called.
    expect(regenerateToken).not.toHaveBeenCalled();

    // Open the modal.
    await userEvent.click(screen.getByRole('button', { name: /regenerate token/i }));
    await waitFor(() => screen.getByText('Regenerate pairing token'));

    // Still not called — the modal is the gate.
    expect(regenerateToken).not.toHaveBeenCalled();

    // Confirm inside the modal.
    await userEvent.click(screen.getByRole('button', { name: 'Regenerate' }));

    await waitFor(() => {
      expect(regenerateToken).toHaveBeenCalledTimes(1);
    });
  });

  it('closes the ConfirmModal without calling regenerateToken when Cancel is clicked', async () => {
    const regenerateToken = vi.fn().mockResolvedValue({ token: 'tok-new' });
    renderWithClient({
      'extensionBridge.status': vi.fn().mockResolvedValue({
        port: 9712,
        connected: true,
        lastSeenMs: null,
        token: 'tok-abc123',
      }),
      'extensionBridge.regenerateToken': regenerateToken,
    });

    await waitFor(() => screen.getByRole('button', { name: /regenerate token/i }));
    await userEvent.click(screen.getByRole('button', { name: /regenerate token/i }));
    await waitFor(() => screen.getByText('Regenerate pairing token'));

    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    // Mutation must never have been called.
    expect(regenerateToken).not.toHaveBeenCalled();
  });
});
