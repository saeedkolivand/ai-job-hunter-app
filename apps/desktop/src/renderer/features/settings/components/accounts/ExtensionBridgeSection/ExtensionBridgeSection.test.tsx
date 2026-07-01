import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ExtensionBridgeStatus } from '@ajh/shared';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { ExtensionBridgeSection } from './index';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderSection(
  statusPayload: ExtensionBridgeStatus = { port: 9712, connected: true, token: 'tok-abc123' },
  regenerateImpl: () => Promise<unknown> = () => Promise.resolve({ token: 'tok-new' })
) {
  const client = createMockClient({
    'extensionBridge.status': vi.fn().mockResolvedValue(statusPayload),
    'extensionBridge.regenerateToken': vi.fn().mockImplementation(regenerateImpl),
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

  const result = render(<ExtensionBridgeSection />, { wrapper: Wrapper });
  return { ...result, client, queryClient };
}

// ---------------------------------------------------------------------------
// clipboard stub
// ---------------------------------------------------------------------------

beforeEach(() => {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ExtensionBridgeSection', () => {
  it('displays the pairing token value from the hook', async () => {
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' });

    // The token is rendered in a read-only Input — query by its value attribute.
    await waitFor(() => {
      const input = screen.getByRole<HTMLInputElement>('textbox');
      expect(input.value).toBe('tok-abc123');
    });
  });

  it('displays the port number from the hook', async () => {
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByText('9712')).toBeInTheDocument();
    });
  });

  it('shows the connected pill when status.connected is true', async () => {
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' });

    // The translated "Connected" label (not the raw key).
    await waitFor(() => {
      expect(screen.getByText('Connected')).toBeInTheDocument();
    });
  });

  it('shows the disconnected pill when status.connected is false', async () => {
    renderSection({ port: 9712, connected: false, token: 'tok-abc123' });

    await waitFor(() => {
      expect(screen.getByText('Not connected')).toBeInTheDocument();
    });
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
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' });

    await waitFor(() => screen.getByRole('button', { name: /copy/i }));
    await userEvent.click(screen.getByRole('button', { name: /copy/i }));

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('tok-abc123');
  });

  it('does not call clipboard.writeText when the token is empty', async () => {
    renderSection({ port: null, connected: false, token: '' });

    await waitFor(() => screen.getByRole('button', { name: /copy/i }));
    // The Copy button is disabled when token is empty — click should be a no-op.
    const btn = screen.getByRole('button', { name: /copy/i });
    expect(btn).toBeDisabled();

    // Even if somehow triggered, clipboard must not be called.
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
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
    const client = createMockClient({
      'extensionBridge.status': vi.fn().mockResolvedValue({
        port: 9712,
        connected: true,
        token: 'tok-abc123',
      }),
      'extensionBridge.regenerateToken': regenerateToken,
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

    render(<ExtensionBridgeSection />, { wrapper: Wrapper });

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
    const client = createMockClient({
      'extensionBridge.status': vi.fn().mockResolvedValue({
        port: 9712,
        connected: true,
        token: 'tok-abc123',
      }),
      'extensionBridge.regenerateToken': regenerateToken,
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

    render(<ExtensionBridgeSection />, { wrapper: Wrapper });

    await waitFor(() => screen.getByRole('button', { name: /regenerate token/i }));
    await userEvent.click(screen.getByRole('button', { name: /regenerate token/i }));
    await waitFor(() => screen.getByText('Regenerate pairing token'));

    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    // Mutation must never have been called.
    expect(regenerateToken).not.toHaveBeenCalled();
  });
});
