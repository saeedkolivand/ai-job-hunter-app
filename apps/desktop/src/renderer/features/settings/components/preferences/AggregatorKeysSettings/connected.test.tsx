/**
 * AggregatorKeysSettings — connected state.
 *
 *  - stored-key badge shown; Remove button present.
 *  - clicking Remove opens the confirm modal.
 *  - confirming Remove calls removeProviderKey.
 *  - Remove does NOT call mutateAsync when removeProviderKey.isPending (re-entrancy guard).
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { PROVIDER_SLOTS } from '@ajh/shared';

import { AggregatorKeysSettings } from './index';
import {
  firstRemoveButton,
  installResetHooks,
  mockNotify,
  mockRemoveMutateAsync,
  modalConfirmButton,
  state,
} from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-support')).translationsMock);
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-support')).uiMock(importOriginal)
);
vi.mock('@/services', async () => (await import('./test-support')).servicesMock);

installResetHooks();

describe('AggregatorKeysSettings — connected state', () => {
  it('shows the stored-key badge when a slot has a key', () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;

    render(<AggregatorKeysSettings />);

    expect(screen.getByText('settings.aggregatorKeys.adzunaAppId.connected')).toBeInTheDocument();
  });

  it('shows a Remove button when a slot has a key', () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;

    render(<AggregatorKeysSettings />);

    expect(
      screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.remove/i }).length
    ).toBeGreaterThanOrEqual(1);
  });

  it('clicking Remove opens the confirm modal', async () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    await user.click(firstRemoveButton());

    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('calls removeProviderKey with the correct slot on modal confirm', async () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    await user.click(firstRemoveButton());
    await user.click(modalConfirmButton());

    await waitFor(() =>
      expect(mockRemoveMutateAsync).toHaveBeenCalledWith({ provider: PROVIDER_SLOTS.adzunaAppId })
    );
  });

  it('does NOT call removeProviderKey.mutateAsync when isPending (remove re-entrancy guard)', async () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    state.removeIsPending = true;
    mockRemoveMutateAsync.mockClear();
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    await user.click(firstRemoveButton());
    fireEvent.click(modalConfirmButton());

    expect(mockRemoveMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the removeError i18n message (not raw error) when remove mutation rejects', async () => {
    state.keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    mockRemoveMutateAsync.mockRejectedValueOnce(new Error('keyring: permission denied'));
    const user = userEvent.setup();

    render(<AggregatorKeysSettings />);

    await user.click(firstRemoveButton());
    await user.click(modalConfirmButton());

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledOnce());
    const [call] = mockNotify.error.mock.calls;
    expect(call?.[0]).toEqual({ message: 'settings.aggregatorKeys.removeError' });
    expect(call?.[0]).not.toMatchObject({ message: expect.stringContaining('keyring') });
  });
});
