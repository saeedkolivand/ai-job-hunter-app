import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { ExtensionBridgeStatus } from '@ajh/shared';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import type * as ServicesModule from '@/services';
import { createMockClient, makeQueryClient } from '@/test-support';

import { ExtensionBridgeSection } from './index';

// The live active provider the renderer would use for `ai_generate` today —
// controlled per-test (mirrors StepAction.test.tsx's pattern) so the ai-assist
// toggle's "snapshot the current provider on enable" behavior is testable
// without a real Zustand `preferences-store`. Partial mock — every OTHER
// `@/services` hook stays real (backed by `createMockClient`).
let stubbedGenerateConfig: { provider: string; model: string; baseUrl: string | undefined } = {
  provider: 'openai',
  model: 'gpt-4o',
  baseUrl: undefined,
};

vi.mock('@/services', async (importOriginal) => {
  const actual = await importOriginal<typeof ServicesModule>();
  return { ...actual, useGenerateConfig: () => stubbedGenerateConfig };
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderSection(
  statusPayload: ExtensionBridgeStatus = { port: 9712, connected: true, token: 'tok-abc123' },
  regenerateImpl: () => Promise<unknown> = () => Promise.resolve({ token: 'tok-new' }),
  autofillEnabled = false,
  setAutofill = vi.fn().mockImplementation((enabled: boolean) => Promise.resolve({ enabled }))
) {
  const client = createMockClient({
    'extensionBridge.status': vi.fn().mockResolvedValue(statusPayload),
    'extensionBridge.regenerateToken': vi.fn().mockImplementation(regenerateImpl),
    'extensionBridge.autofillEnabled': vi.fn().mockResolvedValue({ enabled: autofillEnabled }),
    'extensionBridge.setAutofillEnabled': setAutofill,
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

  it('renders the assisted-autofill switch reflecting the persisted opt-in (default off)', async () => {
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' }, undefined, false);

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /assisted form autofill/i });
      expect(sw).toHaveAttribute('aria-checked', 'false');
    });
  });

  it('reflects an enabled opt-in as a checked switch', async () => {
    renderSection({ port: 9712, connected: true, token: 'tok-abc123' }, undefined, true);

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /assisted form autofill/i });
      expect(sw).toHaveAttribute('aria-checked', 'true');
    });
  });

  it('persists the opt-in when the autofill switch is toggled on', async () => {
    const setAutofill = vi
      .fn()
      .mockImplementation((enabled: boolean) => Promise.resolve({ enabled }));
    renderSection(
      { port: 9712, connected: true, token: 'tok-abc123' },
      undefined,
      false,
      setAutofill
    );

    const sw = await screen.findByRole('switch', { name: /assisted form autofill/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAutofill).toHaveBeenCalledWith(true);
    });
  });

  it('shows the toggleFailed notification when setAutofillEnabled rejects', async () => {
    const setAutofill = vi.fn().mockRejectedValue(new Error('store write failed'));
    renderSection(
      { port: 9712, connected: true, token: 'tok-abc123' },
      undefined,
      false,
      setAutofill
    );

    const sw = await screen.findByRole('switch', { name: /assisted form autofill/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAutofill).toHaveBeenCalledWith(true);
    });
    await waitFor(() => {
      expect(screen.getByText('Could not update the autofill setting.')).toBeInTheDocument();
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

  // -------------------------------------------------------------------------
  // AI answer-assist opt-in — a SEPARATE toggle from autofill above.
  // -------------------------------------------------------------------------

  it('renders the ai-assist switch reflecting the persisted opt-in (default off)', async () => {
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toHaveAttribute('aria-checked', 'false');
    });
  });

  it('snapshots the current active provider/model when the ai-assist switch is turned on', async () => {
    stubbedGenerateConfig = { provider: 'openai', model: 'gpt-4o', baseUrl: undefined };
    const setAiAssist = vi.fn().mockResolvedValue({ enabled: true });
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
      'extensionBridge.setAiAssistEnabled': setAiAssist,
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAiAssist).toHaveBeenCalledWith(true, 'openai', 'gpt-4o', undefined);
    });
  });

  it('sends no provider fields when the ai-assist switch is turned off', async () => {
    const setAiAssist = vi.fn().mockResolvedValue({ enabled: false });
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: true }),
      'extensionBridge.setAiAssistEnabled': setAiAssist,
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAiAssist).toHaveBeenCalledWith(false, undefined, undefined, undefined);
    });
  });

  it('disables the ai-assist switch when no AI provider/model is configured', async () => {
    stubbedGenerateConfig = { provider: 'ollama', model: '', baseUrl: undefined };
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toBeDisabled();
    });
    // Restore the default for later tests in this file.
    stubbedGenerateConfig = { provider: 'openai', model: 'gpt-4o', baseUrl: undefined };
  });

  it('keeps the ai-assist switch enabled for a CLI-agent provider with no model selected (Completer::resolve allows it)', async () => {
    stubbedGenerateConfig = { provider: 'claude-code', model: '', baseUrl: undefined };
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).not.toBeDisabled();
    });
    // Restore the default for later tests in this file.
    stubbedGenerateConfig = { provider: 'openai', model: 'gpt-4o', baseUrl: undefined };
  });

  // HIGH fix: `disabled` must only ever gate the ON direction. Once the
  // opt-in is already enabled, the user must always be able to turn it back
  // off — even if the live provider config becomes unconfigured afterward
  // (e.g. the active provider/model was cleared elsewhere in Settings).
  it('lets an already-enabled ai-assist switch be turned off even if the provider becomes unconfigured', async () => {
    stubbedGenerateConfig = { provider: 'ollama', model: '', baseUrl: undefined };
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi
        .fn()
        .mockResolvedValue({ enabled: true, provider: 'openai', model: 'gpt-4o' }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toHaveAttribute('aria-checked', 'true');
      expect(sw).not.toBeDisabled();
    });
    // Restore the default for later tests in this file.
    stubbedGenerateConfig = { provider: 'openai', model: 'gpt-4o', baseUrl: undefined };
  });

  it('does not disable the ai-assist switch when it is enabled and the provider is configured', async () => {
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi
        .fn()
        .mockResolvedValue({ enabled: true, provider: 'openai', model: 'gpt-4o' }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).not.toBeDisabled();
    });
  });

  it('shows the pinned provider/model snapshot in the description while the opt-in is on', async () => {
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi
        .fn()
        .mockResolvedValue({ enabled: true, provider: 'openai', model: 'gpt-4o' }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => {
      expect(screen.getByText(/Using: OpenAI · gpt-4o/)).toBeInTheDocument();
    });
  });

  it('omits the pinned-snapshot line while the opt-in is off', async () => {
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    await waitFor(() => screen.getByRole('switch', { name: /ai answer drafting/i }));
    expect(screen.queryByText(/Using:/)).not.toBeInTheDocument();
  });

  it('shows the toggleFailed notification when setAiAssistEnabled rejects', async () => {
    const setAiAssist = vi.fn().mockRejectedValue(new Error('store write failed'));
    const client = createMockClient({
      'extensionBridge.status': vi
        .fn()
        .mockResolvedValue({ port: 9712, connected: true, token: 'tok-abc123' }),
      'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue({ enabled: false }),
      'extensionBridge.setAiAssistEnabled': setAiAssist,
    });
    const queryClient = makeQueryClient();
    render(<ExtensionBridgeSection />, {
      wrapper: ({ children }) => (
        <QueryClientProvider client={queryClient}>
          <AppClientProvider client={client}>
            <NotificationProvider>{children}</NotificationProvider>
          </AppClientProvider>
        </QueryClientProvider>
      ),
    });

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(
        screen.getByText('Could not update the AI answer-drafting setting.')
      ).toBeInTheDocument();
    });
  });
});
