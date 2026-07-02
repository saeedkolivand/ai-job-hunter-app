/**
 * AggregatorKeysSettings — focused behaviour tests.
 *
 * Covers:
 *  - not connected: password inputs rendered for all six key fields (incl. Comeet).
 *  - not connected: eye-toggle buttons have accessible names (a11y guard).
 *  - not connected: toggling show/hide changes input type.
 *  - not connected: Save calls setProviderKey after typing a value.
 *  - not connected: Save is a no-op (disabled) when input is blank.
 *  - not connected: Save does NOT call mutateAsync when setProviderKey.isPending (re-entrancy guard).
 *  - connected: stored-key badge shown; Remove button present.
 *  - connected: clicking Remove opens the confirm modal.
 *  - connected: confirming Remove calls removeProviderKey.
 *  - connected: Remove does NOT call mutateAsync when removeProviderKey.isPending (re-entrancy guard).
 *  - Apify LinkedIn section: toggle fires updateScrapingSettings with enabled=true.
 *  - Apify LinkedIn section: toggle error uses i18n key, not raw error.
 *  - Apify LinkedIn section: actor-id Save calls updateScrapingSettings with the trimmed value.
 *  - Apify LinkedIn section: actor-id Save is re-entrancy guarded (isPending).
 *  - Comeet section: both credential field labels render (company UID + API token).
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
let updateScrapingIsPending = false;

// ── mutable Apify settings so tests can control the initial state ─────────

let mockScrapingData: { apifyLinkedinEnabled: boolean; apifyLinkedinActorId?: string } = {
  apifyLinkedinEnabled: false,
  apifyLinkedinActorId: undefined,
};

afterEach(() => {
  for (const k of Object.keys(keyState)) delete keyState[k];
  setIsPending = false;
  removeIsPending = false;
  updateScrapingIsPending = false;
  mockScrapingData = { apifyLinkedinEnabled: false, apifyLinkedinActorId: undefined };
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
const mockUpdateScrapingMutateAsync = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useHasProviderKey: (slot: string) => ({ data: { has: keyState[slot] ?? false } }),
  useSetProviderKey: () => ({ mutateAsync: mockSetMutateAsync, isPending: setIsPending }),
  useRemoveProviderKey: () => ({ mutateAsync: mockRemoveMutateAsync, isPending: removeIsPending }),
  useOpenExternal: () => ({ mutateAsync: vi.fn() }),
  useScrapingSettings: () => ({ data: mockScrapingData }),
  useUpdateScrapingSettings: () => ({
    mutateAsync: mockUpdateScrapingMutateAsync,
    isPending: updateScrapingIsPending,
  }),
}));

// ── component under test ───────────────────────────────────────────────────

import { AggregatorKeysSettings } from './index';

// ── helpers ────────────────────────────────────────────────────────────────

function getPasswordInputs(container: HTMLElement) {
  return Array.from(container.querySelectorAll('input[type="password"]'));
}

// ── tests — not connected (default keyState = all false) ──────────────────

describe('AggregatorKeysSettings — not connected', () => {
  it('renders password inputs for all six key fields (incl. Comeet)', () => {
    const { container } = render(<AggregatorKeysSettings />);
    expect(getPasswordInputs(container).length).toBe(6);
  });

  it('Save buttons are disabled when inputs are empty', () => {
    render(<AggregatorKeysSettings />);
    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    saveButtons.forEach((btn) => expect(btn).toBeDisabled());
  });

  it('eye-toggle buttons have accessible names for all six fields', () => {
    render(<AggregatorKeysSettings />);
    const eyeToggles = screen.getAllByRole('button', {
      name: 'settings.aiProvider.showKey',
    });
    expect(eyeToggles.length).toBe(6);
  });

  it('toggling the first eye-button switches that field from password to text', async () => {
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    expect(getPasswordInputs(container).length).toBe(6);

    const toggles = screen.getAllByRole('button', { name: 'settings.aiProvider.showKey' });
    const firstToggle = toggles[0];
    if (!firstToggle) throw new Error('No eye-toggle found');
    await user.click(firstToggle);

    expect(getPasswordInputs(container).length).toBe(5);
    // actor-id input is always type="text"; toggled credential input adds a second.
    expect(Array.from(container.querySelectorAll('input[type="text"]')).length).toBe(2);
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
    setIsPending = true;
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    const inputs = getPasswordInputs(container);
    const firstInput = inputs[0];
    if (!firstInput) throw new Error('No password input found');
    await user.type(firstInput, 'pending-key');

    await user.keyboard('{Enter}');

    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const firstSave = saveButtons[0];
    if (!firstSave) throw new Error('No Save button found');
    await user.click(firstSave);

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
    keyState[PROVIDER_SLOTS.adzunaAppId] = true;
    removeIsPending = true;
    mockRemoveMutateAsync.mockClear();
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
    fireEvent.click(confirmBtn);

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
    expect(call?.[0]).not.toMatchObject({ message: expect.stringContaining('keyring') });
  });
});

// ── tests — Apify LinkedIn section ────────────────────────────────────────

describe('AggregatorKeysSettings — Apify LinkedIn section', () => {
  it('renders the enable toggle when scrapingSettings are loaded', () => {
    render(<AggregatorKeysSettings />);
    expect(
      screen.getByRole('switch', {
        name: 'settings.aggregatorKeys.apifyLinkedin.enabledLabel',
      })
    ).toBeInTheDocument();
  });

  it('toggle fires updateScrapingSettings with enabled=true', async () => {
    mockUpdateScrapingMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const toggle = screen.getByRole('switch', {
      name: 'settings.aggregatorKeys.apifyLinkedin.enabledLabel',
    });
    await user.click(toggle);

    await waitFor(() =>
      expect(mockUpdateScrapingMutateAsync).toHaveBeenCalledWith({ apifyLinkedinEnabled: true })
    );
  });

  it('toggle error shows apifyLinkedin.saveError i18n key (not raw error)', async () => {
    mockUpdateScrapingMutateAsync.mockRejectedValueOnce(new Error('store write failed'));
    mockNotify.error.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const toggle = screen.getByRole('switch', {
      name: 'settings.aggregatorKeys.apifyLinkedin.enabledLabel',
    });
    await user.click(toggle);

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledOnce());
    const [call] = mockNotify.error.mock.calls;
    expect(call?.[0]).toEqual({
      message: 'settings.aggregatorKeys.apifyLinkedin.saveError',
    });
  });

  it('actor-id Save calls updateScrapingSettings with the typed value', async () => {
    mockUpdateScrapingMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const actorInput = screen.getByRole('textbox', {
      name: 'settings.aggregatorKeys.apifyLinkedin.actorIdLabel',
    });
    await user.type(actorInput, 'my~actor');

    // The actor-id Save button lives in the same row as the input — scope the
    // query to that row instead of an array position (fragile now that the
    // Comeet fields render more Save buttons after this one).
    const actorSave = actorInput.closest('div')?.querySelector('button');
    if (!actorSave) throw new Error('No actor-id Save button found');
    await user.click(actorSave);

    await waitFor(() =>
      expect(mockUpdateScrapingMutateAsync).toHaveBeenCalledWith({
        apifyLinkedinActorId: 'my~actor',
      })
    );
  });

  it('actor-id Save is re-entrancy guarded when isPending', async () => {
    updateScrapingIsPending = true;
    mockUpdateScrapingMutateAsync.mockClear();
    const user = userEvent.setup();
    render(<AggregatorKeysSettings />);

    const actorInput = screen.getByRole('textbox', {
      name: 'settings.aggregatorKeys.apifyLinkedin.actorIdLabel',
    });
    await user.type(actorInput, 'blocked~actor');
    await user.keyboard('{Enter}');

    expect(mockUpdateScrapingMutateAsync).not.toHaveBeenCalled();
  });

  it('renders the cost warning notice', () => {
    render(<AggregatorKeysSettings />);
    expect(
      screen.getByText('settings.aggregatorKeys.apifyLinkedin.costWarning')
    ).toBeInTheDocument();
  });
});

// ── tests — Comeet section ─────────────────────────────────────────────────

describe('AggregatorKeysSettings — Comeet section', () => {
  it('renders both the company UID and API token field labels', () => {
    render(<AggregatorKeysSettings />);
    expect(screen.getByText('settings.aggregatorKeys.comeetCompanyUid.label')).toBeInTheDocument();
    expect(screen.getByText('settings.aggregatorKeys.comeetApiToken.label')).toBeInTheDocument();
  });

  it('calls setProviderKey with the Comeet company-UID slot on Save', async () => {
    mockSetMutateAsync.mockClear();
    const user = userEvent.setup();
    const { container } = render(<AggregatorKeysSettings />);

    // Comeet fields are the last two password inputs (rendered after Apify).
    const inputs = getPasswordInputs(container);
    const companyUidInput = inputs[inputs.length - 2];
    if (!companyUidInput) throw new Error('No Comeet company-UID input found');
    await user.type(companyUidInput, 'my-company-uid');

    const saveButtons = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i });
    const companyUidSave = saveButtons[saveButtons.length - 2];
    if (!companyUidSave) throw new Error('No Comeet company-UID Save button found');
    await user.click(companyUidSave);

    await waitFor(() =>
      expect(mockSetMutateAsync).toHaveBeenCalledWith({
        provider: PROVIDER_SLOTS.comeetCompanyUid,
        apiKey: 'my-company-uid',
      })
    );
  });
});
