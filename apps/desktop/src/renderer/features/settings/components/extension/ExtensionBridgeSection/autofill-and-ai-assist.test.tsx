import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { generateConfig, installBridgeHooks, renderSection } from './test-support';

vi.mock('@/services', async (importOriginal) =>
  (await import('./services-stub')).servicesMock(importOriginal)
);

installBridgeHooks();

describe('ExtensionBridgeSection', () => {
  it('renders the assisted-autofill switch reflecting the persisted opt-in (default off)', async () => {
    renderSection(
      { port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' },
      undefined,
      false
    );

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /assisted form autofill/i });
      expect(sw).toHaveAttribute('aria-checked', 'false');
    });
  });

  it('reflects an enabled opt-in as a checked switch', async () => {
    renderSection(
      { port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' },
      undefined,
      true
    );

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
      { port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' },
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
      { port: 9712, connected: true, lastSeenMs: null, token: 'tok-abc123' },
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

  // -------------------------------------------------------------------------
  // AI answer-assist opt-in — a SEPARATE toggle from autofill above.
  // -------------------------------------------------------------------------

  it('renders the ai-assist switch reflecting the persisted opt-in (default off)', async () => {
    renderSection(undefined, undefined, undefined, undefined, { enabled: false });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toHaveAttribute('aria-checked', 'false');
    });
  });

  it('sends just the enabled flag when the ai-assist switch is turned on (no provider snapshot)', async () => {
    const setAiAssist = vi.fn().mockResolvedValue({ enabled: true });
    renderSection(undefined, undefined, undefined, undefined, { enabled: false }, setAiAssist);

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    // A draft resolves the active provider from the backend store at
    // answer-time (task #16), so the toggle threads nothing but the flag.
    await waitFor(() => {
      expect(setAiAssist).toHaveBeenCalledWith(true);
    });
  });

  it('sends just the enabled flag when the ai-assist switch is turned off', async () => {
    const setAiAssist = vi.fn().mockResolvedValue({ enabled: false });
    renderSection(undefined, undefined, undefined, undefined, { enabled: true }, setAiAssist);

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(setAiAssist).toHaveBeenCalledWith(false);
    });
  });

  it('disables the ai-assist switch when no AI provider/model is configured', async () => {
    generateConfig.current = { provider: 'ollama', model: '' };
    renderSection(undefined, undefined, undefined, undefined, { enabled: false });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toBeDisabled();
    });
    // (c) providerConfigured === false must show the noProvider description —
    // regardless of the opt-in's own enabled state (see index.tsx's `description`).
    expect(
      screen.getByText('Choose an AI provider in Settings → AI first, then turn this on.')
    ).toBeInTheDocument();
  });

  it('keeps the ai-assist switch enabled for a CLI-agent provider with no model selected (Completer::resolve allows it)', async () => {
    generateConfig.current = { provider: 'claude-code', model: '' };
    renderSection(undefined, undefined, undefined, undefined, { enabled: false });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).not.toBeDisabled();
    });
  });

  // HIGH fix: `disabled` must only ever gate the ON direction. Once the
  // opt-in is already enabled, the user must always be able to turn it back
  // off — even if the live provider config becomes unconfigured afterward
  // (e.g. the active provider/model was cleared elsewhere in Settings).
  it('lets an already-enabled ai-assist switch be turned off even if the provider becomes unconfigured', async () => {
    generateConfig.current = { provider: 'ollama', model: '' };
    renderSection(undefined, undefined, undefined, undefined, { enabled: true });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).toHaveAttribute('aria-checked', 'true');
      expect(sw).not.toBeDisabled();
    });
  });

  it('does not disable the ai-assist switch when it is enabled and the provider is configured', async () => {
    renderSection(undefined, undefined, undefined, undefined, { enabled: true });

    await waitFor(() => {
      const sw = screen.getByRole('switch', { name: /ai answer drafting/i });
      expect(sw).not.toBeDisabled();
    });
  });

  it('shows the active provider/model (from the backend store) in the description while the opt-in is on', async () => {
    renderSection(undefined, undefined, undefined, undefined, { enabled: true }, undefined, {
      activeProvider: 'openai',
      model: 'gpt-4o',
      providers: {},
    });

    await waitFor(() => {
      expect(screen.getByText(/Using: OpenAI · gpt-4o/)).toBeInTheDocument();
    });
  });

  it('omits the "Using:" line while the opt-in is off, even with an active provider', async () => {
    renderSection(undefined, undefined, undefined, undefined, { enabled: false }, undefined, {
      activeProvider: 'openai',
      model: 'gpt-4o',
      providers: {},
    });

    await waitFor(() => screen.getByRole('switch', { name: /ai answer drafting/i }));
    expect(screen.queryByText(/Using:/)).not.toBeInTheDocument();
  });

  it('shows the toggleFailed notification when setAiAssistEnabled rejects', async () => {
    const setAiAssist = vi.fn().mockRejectedValue(new Error('store write failed'));
    renderSection(undefined, undefined, undefined, undefined, { enabled: false }, setAiAssist);

    const sw = await screen.findByRole('switch', { name: /ai answer drafting/i });
    await userEvent.click(sw);

    await waitFor(() => {
      expect(
        screen.getByText('Could not update the AI answer-drafting setting.')
      ).toBeInTheDocument();
    });
  });
});
