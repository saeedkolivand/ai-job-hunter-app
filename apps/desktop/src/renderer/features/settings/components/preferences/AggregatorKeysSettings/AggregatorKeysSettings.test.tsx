/**
 * AggregatorKeysSettings — focused behaviour tests (not-connected state + Comeet).
 *
 * Covers:
 *  - not connected: password inputs rendered for all seven key fields (incl. Jooble, Comeet).
 *  - not connected: eye-toggle buttons have accessible names (a11y guard).
 *  - not connected: toggling show/hide changes input type.
 *  - not connected: Save calls setProviderKey after typing a value.
 *  - not connected: Save is a no-op (disabled) when input is blank.
 *  - not connected: Save does NOT call mutateAsync when setProviderKey.isPending (re-entrancy guard).
 *  - Comeet section: both credential field labels render (company UID + API token).
 *
 * Connected state → `connected.test.tsx`; Apify LinkedIn → `apify-linkedin.test.tsx`.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { PROVIDER_SLOTS } from '@ajh/shared';

import { AggregatorKeysSettings } from './index';
import {
  firstPasswordInput,
  firstSaveButton,
  getPasswordInputs,
  installResetHooks,
  mockNotify,
  mockSetMutateAsync,
  state,
} from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-support')).translationsMock);
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-support')).uiMock(importOriginal)
);
vi.mock('@/services', async () => (await import('./test-support')).servicesMock);

installResetHooks();

describe('AggregatorKeysSettings — not connected', () => {
  it('renders password inputs for all seven key fields (incl. Jooble, Comeet)', () => {
    const { container } = render(<AggregatorKeysSettings />);
    expect(getPasswordInputs(container).length).toBe(7);
  });

  it('Save buttons are disabled when inputs are empty', () => {
    render(<AggregatorKeysSettings />);
    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    saveButtons.forEach((btn) => expect(btn).toBeDisabled());
  });

  it('eye-toggle buttons have accessible names for all seven fields', () => {
    render(<AggregatorKeysSettings />);
    const eyeToggles = screen.getAllByRole('button', {
      name: 'settings.aiProvider.showKey',
    });
    expect(eyeToggles.length).toBe(7);
  });

  it('toggling the first eye-button switches that field from password to text', async () => {
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    expect(getPasswordInputs(container).length).toBe(7);

    const toggles = screen.getAllByRole('button', { name: 'settings.aiProvider.showKey' });
    const firstToggle = toggles[0];
    if (!firstToggle) throw new Error('No eye-toggle found');
    await user.click(firstToggle);

    expect(getPasswordInputs(container).length).toBe(6);
    // actor-id input is always type="text"; toggled credential input adds a second.
    expect(Array.from(container.querySelectorAll('input[type="text"]')).length).toBe(2);
  });

  it('renders the Jooble field with its "get a free key" docs link', () => {
    render(<AggregatorKeysSettings />);
    expect(screen.getByText('settings.aggregatorKeys.joobleKey.label')).toBeInTheDocument();
    expect(screen.getByText('jooble.org/api/about')).toBeInTheDocument();
  });

  it('calls setProviderKey with the Jooble slot on Save', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const label = screen.getByText('settings.aggregatorKeys.joobleKey.label');
    const row = label.parentElement;
    if (!row) throw new Error('Jooble field row not found');

    const joobleInput = within(row).getByPlaceholderText(
      'settings.aggregatorKeys.joobleKey.placeholder'
    );
    await user.type(joobleInput, 'my-jooble-key');

    const joobleSave = within(row).getByRole('button', {
      name: /settings\.aggregatorKeys\.save/i,
    });
    await user.click(joobleSave);

    await waitFor(() =>
      expect(mockSetMutateAsync).toHaveBeenCalledWith({
        provider: PROVIDER_SLOTS.joobleKey,
        apiKey: 'my-jooble-key',
      })
    );
  });

  it('calls setProviderKey with the correct slot and value on Save', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    await user.type(firstPasswordInput(container), 'my-app-id');
    await user.click(firstSaveButton());

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

    await user.click(firstSaveButton());

    expect(mockSetMutateAsync).not.toHaveBeenCalled();
  });

  it('does NOT call setProviderKey.mutateAsync when isPending (save re-entrancy guard)', async () => {
    state.setIsPending = true;
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    await user.type(firstPasswordInput(container), 'pending-key');

    await user.keyboard('{Enter}');

    await user.click(firstSaveButton());

    expect(mockSetMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the generic saveError i18n message (not raw error) when save mutation rejects', async () => {
    mockSetMutateAsync.mockRejectedValueOnce(
      new Error('keyring: /home/user/.local/share/keyrings/secret')
    );
    mockNotify.error.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    await user.type(firstPasswordInput(container), 'bad-key');
    await user.click(firstSaveButton());

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledOnce());
    const [call] = mockNotify.error.mock.calls;
    expect(call?.[0]).toEqual({ message: 'settings.aggregatorKeys.saveError' });
    expect(call?.[0]).not.toMatchObject({ message: expect.stringContaining('keyring') });
  });
});

describe('AggregatorKeysSettings — Comeet section', () => {
  it('renders both the company UID and API token field labels', () => {
    render(<AggregatorKeysSettings />);
    expect(screen.getByText('settings.aggregatorKeys.comeetCompanyUid.label')).toBeInTheDocument();
    expect(screen.getByText('settings.aggregatorKeys.comeetApiToken.label')).toBeInTheDocument();
  });

  it('calls setProviderKey with the Comeet company-UID slot on Save', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    // Scope to the field's own row via its rendered label — not a positional
    // index, which silently breaks if a field is ever appended after Comeet.
    const label = screen.getByText('settings.aggregatorKeys.comeetCompanyUid.label');
    const row = label.parentElement;
    if (!row) throw new Error('Comeet company-UID field row not found');

    const companyUidInput = within(row).getByPlaceholderText(
      'settings.aggregatorKeys.comeetCompanyUid.placeholder'
    );
    await user.type(companyUidInput, 'my-company-uid');

    const companyUidSave = within(row).getByRole('button', {
      name: /settings\.aggregatorKeys\.save/i,
    });
    await user.click(companyUidSave);

    await waitFor(() =>
      expect(mockSetMutateAsync).toHaveBeenCalledWith({
        provider: PROVIDER_SLOTS.comeetCompanyUid,
        apiKey: 'my-company-uid',
      })
    );
  });
});
