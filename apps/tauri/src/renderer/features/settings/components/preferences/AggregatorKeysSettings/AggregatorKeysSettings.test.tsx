/**
 * AggregatorKeysSettings — focused behaviour tests.
 *
 * Covers:
 *  - not connected: password inputs rendered; Save buttons disabled when empty.
 *  - not connected: eye-toggle buttons have accessible names (a11y guard).
 *  - not connected: toggling show/hide changes input type.
 *  - not connected: Save calls setProviderKey after typing a value.
 *  - not connected: Save is a no-op (disabled) when input is blank.
 *  - not connected: Save does NOT call mutateAsync when setProviderKey.isPending (re-entrancy guard).
 *  - connected: stored-key badge shown; Remove button present.
 *  - connected: clicking Remove opens the confirm modal.
 *  - connected: confirming Remove calls removeProviderKey.
 *  - connected: Remove does NOT call mutateAsync when removeProviderKey.isPending (re-entrancy guard).
 *
 * Service hooks are stubbed at the boundary; the real @ajh/ui tree is used
 * (only useNotification is overridden to avoid a Notification provider).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { PROVIDER_SLOTS } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

// ── mutable key-state so tests can flip connected/disconnected per slot ────

const keyState: Record<string, boolean> = {};

// ── per-test pending-state overrides ──────────────────────────────────────
// Default: not pending. Tests that probe the re-entrancy guard flip these.

let setIsPending = false;
let removeIsPending = false;

afterEach(() => {
  for (const k of Object.keys(keyState)) delete keyState[k];
  setIsPending = false;
  removeIsPending = false;
  vi.clearAllMocks();
});

// ── i18n stub ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @ajh/ui — use the real library, override only useNotification ──────────

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => mockNotify,
  };
});

// ── service stubs ──────────────────────────────────────────────────────────

const mockSetMutateAsync = vi.fn().mockResolvedValue(undefined);
const mockRemoveMutateAsync = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useHasProviderKey: (slot: string) => ({ data: { has: keyState[slot] ?? false } }),
  useSetProviderKey: () => ({ mutateAsync: mockSetMutateAsync, isPending: setIsPending }),
  useRemoveProviderKey: () => ({ mutateAsync: mockRemoveMutateAsync, isPending: removeIsPending }),
  useOpenExternal: () => ({ mutateAsync: vi.fn() }),
}));

// ── component under test ───────────────────────────────────────────────────

import { AggregatorKeysSettings } from './index';

// ── helpers ────────────────────────────────────────────────────────────────

function getPasswordInputs(container: HTMLElement) {
  return Array.from(container.querySelectorAll('input[type="password"]'));
}

// ── tests — not connected (default keyState = all false) ──────────────────

describe('AggregatorKeysSettings — not connected', () => {
  it('renders password inputs for all three key fields', () => {
    const { container } = render(<AggregatorKeysSettings />);
    expect(getPasswordInputs(container).length).toBe(3);
  });

  it('Save buttons are disabled when inputs are empty', () => {
    render(<AggregatorKeysSettings />);
    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    saveButtons.forEach((btn) => expect(btn).toBeDisabled());
  });

  it('eye-toggle buttons have accessible names for all three fields', () => {
    render(<AggregatorKeysSettings />);
    const eyeToggles = screen.getAllByRole('button', {
      name: 'settings.aiProvider.showKey',
    });
    expect(eyeToggles.length).toBe(3);
  });

  it('toggling the first eye-button switches that field from password to text', async () => {
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    expect(getPasswordInputs(container).length).toBe(3);

    const toggles = screen.getAllByRole('button', { name: 'settings.aiProvider.showKey' });
    const firstToggle = toggles[0];
    if (!firstToggle) throw new Error('No eye-toggle found');
    await user.click(firstToggle);

    expect(getPasswordInputs(container).length).toBe(2);
    expect(Array.from(container.querySelectorAll('input[type="text"]')).length).toBe(1);
  });

  it('calls setProviderKey with the correct slot and value on Save', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    const inputs = getPasswordInputs(container);
    const firstInput = inputs[0];
    if (!firstInput) throw new Error('No password input found');
    await user.type(firstInput, 'my-app-id');

    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const firstSave = saveButtons[0];
    if (!firstSave) throw new Error('No Save button found');
    await user.click(firstSave);

    await waitFor(() =>
      expect(mockSetMutateAsync).toHaveBeenCalledWith({
        provider: PROVIDER_SLOTS.adzunaAppId,
        apiKey: 'my-app-id',
      })
    );
  });

  it('does NOT call setProviderKey when Save is disabled (empty input)', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const firstSave = saveButtons[0];
    if (!firstSave) throw new Error('No Save button found');
    await user.click(firstSave);

    expect(mockSetMutateAsync).not.toHaveBeenCalled();
  });

  it('does NOT call setProviderKey.mutateAsync when isPending (save re-entrancy guard)', async () => {
    // Simulate a mutation already in-flight so handleSave's early-return fires.
    setIsPending = true;
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    const inputs = getPasswordInputs(container);
    const firstInput = inputs[0];
    if (!firstInput) throw new Error('No password input found');
    // Type a value so the blank-input guard doesn't fire instead.
    await user.type(firstInput, 'pending-key');

    // Trigger via Enter key (the other branch of the save path).
    await user.keyboard('{Enter}');

    // The Save button click path:
    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const firstSave = saveButtons[0];
    if (!firstSave) throw new Error('No Save button found');
    await user.click(firstSave);

    // Neither trigger should have reached mutateAsync.
    expect(mockSetMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the generic saveError i18n message (not raw error) when save mutation rejects', async () => {
    mockSetMutateAsync.mockRejectedValueOnce(
      new Error('keyring: /home/user/.local/share/keyrings/secret')
    );
    mockNotify.error.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    const inputs = getPasswordInputs(container);
    const firstInput = inputs[0];
    if (!firstInput) throw new Error('No password input found');
    await user.type(firstInput, 'bad-key');

    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const firstSave = saveButtons[0];
    if (!firstSave) throw new Error('No Save button found');
    await user.click(firstSave);

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledOnce());
    const [call] = mockNotify.error.mock.calls;
    expect(call?.[0]).toEqual({ message: 'settings.aggregatorKeys.saveError' });
    // raw error text must never reach the notification
    expect(call?.[0]).not.toMatchObject({ message: expect.stringContaining('keyring') });
  });
});

// ── tests — connected state ────────────────────────────────────────────────

describe('AggregatorKeysSettings — connected state', () => {
  it('shows the stored-key badge when a slot has a key', () => {
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;

    render(<AggregatorKeysSettings />);

    expect(screen.getByText('settings.aggregatorKeys.adzunaAppId.connected')).toBeInTheDocument();
  });

  it('shows a Remove button when a slot has a key', () => {
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;

    render(<AggregatorKeysSettings />);

    expect(
      screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.remove/i }).length
    ).toBeGreaterThanOrEqual(1);
  });

  it('clicking Remove opens the confirm modal', async () => {
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    const removeButtons = screen.getAllByRole('button', {
      name: /settings\.aggregatorKeys\.remove/i,
    });
    const firstRemove = removeButtons[0];
    if (!firstRemove) throw new Error('No Remove button found');
    await user.click(firstRemove);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('calls removeProviderKey with the correct slot on modal confirm', async () => {
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    const removeButtons = screen.getAllByRole('button', {
      name: /settings\.aggregatorKeys\.remove/i,
    });
    const firstRemove = removeButtons[0];
    if (!firstRemove) throw new Error('No Remove button found');
    await user.click(firstRemove);

    // ConfirmModal is open — find its confirm button inside the dialog
    const dialog = screen.getByRole('dialog');
    const confirmBtn = Array.from(dialog.querySelectorAll('button')).find((b) =>
      /settings\.aggregatorKeys\.remove/i.test(b.textContent ?? '')
    );
    if (!confirmBtn) throw new Error('Confirm button not found in modal');
    await user.click(confirmBtn);

    await waitFor(() =>
      expect(mockRemoveMutateAsync).toHaveBeenCalledWith({ provider: PROVIDER_SLOTS.adzunaAppId })
    );
  });

  it('does NOT call removeProviderKey.mutateAsync when isPending (remove re-entrancy guard)', async () => {
    // Put a slot in connected state so the Remove button renders.
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    // Simulate a removal already in-flight so handleRemove's early-return fires.
    removeIsPending = true;
    mockRemoveMutateAsync.mockClear();
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    // Open the confirm modal via the Remove trigger button.
    const removeButtons = screen.getAllByRole('button', {
      name: /settings\.aggregatorKeys\.remove/i,
    });
    const firstRemove = removeButtons[0];
    if (!firstRemove) throw new Error('No Remove button found');
    await user.click(firstRemove);

    // Confirm modal is open. The confirm button is disabled (isConfirming=true),
    // so userEvent ignores it. Use fireEvent to bypass the disabled attribute and
    // directly invoke the onClick handler — this is the scenario the re-entrancy
    // guard defends against (programmatic / rapid double-submit while in-flight).
    const dialog = screen.getByRole('dialog');
    const confirmBtn = Array.from(dialog.querySelectorAll('button')).find((b) =>
      /settings\.aggregatorKeys\.remove/i.test(b.textContent ?? '')
    );
    if (!confirmBtn) throw new Error('Confirm button not found in modal');
    fireEvent.click(confirmBtn);

    // handleRemove returned early at the isPending guard — mutateAsync must not fire.
    expect(mockRemoveMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the removeError i18n message (not raw error) when remove mutation rejects', async () => {
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    mockRemoveMutateAsync.mockRejectedValueOnce(new Error('keyring: permission denied'));
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    const removeButtons = screen.getAllByRole('button', {
      name: /settings\.aggregatorKeys\.remove/i,
    });
    const firstRemove = removeButtons[0];
    if (!firstRemove) throw new Error('No Remove button found');
    await user.click(firstRemove);

    const dialog = screen.getByRole('dialog');
    const confirmBtn = Array.from(dialog.querySelectorAll('button')).find((b) =>
      /settings\.aggregatorKeys\.remove/i.test(b.textContent ?? '')
    );
    if (!confirmBtn) throw new Error('Confirm button not found in modal');
    await user.click(confirmBtn);

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledOnce());
    const [call] = mockNotify.error.mock.calls;
    expect(call?.[0]).toEqual({ message: 'settings.aggregatorKeys.removeError' });
    // raw error text must never reach the notification
    expect(call?.[0]).not.toMatchObject({ message: expect.stringContaining('keyring') });
  });
});
