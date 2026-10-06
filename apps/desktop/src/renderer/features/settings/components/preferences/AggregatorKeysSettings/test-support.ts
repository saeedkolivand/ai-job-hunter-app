/**
 * Shared stubs for the AggregatorKeysSettings suites (no `.test.` in the name, so
 * vitest does not collect it). Service hooks are stubbed at the boundary; the real
 * @ajh/ui tree is used (only useNotification is overridden to avoid a Notification
 * provider). The `vi.mock` calls stay in each suite (they only hoist there) and load
 * the factories below lazily.
 */
import { afterEach, type Mock, vi } from 'vitest';
import { screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

/** Mutable per-test state the service stubs read, so tests can flip it. */
export const state: {
  /** connected/disconnected per slot */
  keyState: Record<string, boolean>;
  // Default: not pending. Tests that probe the re-entrancy guard flip these.
  setIsPending: boolean;
  removeIsPending: boolean;
  updateScrapingIsPending: boolean;
  scraping: { apifyLinkedinEnabled: boolean; apifyLinkedinActorId?: string };
} = {
  keyState: {},
  setIsPending: false,
  removeIsPending: false,
  updateScrapingIsPending: false,
  scraping: { apifyLinkedinEnabled: false, apifyLinkedinActorId: undefined },
};

export const mockNotify: Record<
  'open' | 'success' | 'error' | 'info' | 'warning' | 'destroy',
  Mock
> = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

export const mockSetMutateAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockRemoveMutateAsync: Mock = vi.fn().mockResolvedValue(undefined);
export const mockUpdateScrapingMutateAsync: Mock = vi.fn().mockResolvedValue(undefined);

export const translationsMock = { useTranslation: () => ({ t: (k: string) => k }) };

export const servicesMock: Record<string, unknown> = {
  useHasProviderKey: (slot: string) => ({ data: { has: state.keyState[slot] ?? false } }),
  useSetProviderKey: () => ({ mutateAsync: mockSetMutateAsync, isPending: state.setIsPending }),
  useRemoveProviderKey: () => ({
    mutateAsync: mockRemoveMutateAsync,
    isPending: state.removeIsPending,
  }),
  useOpenExternal: () => ({ mutateAsync: vi.fn() }),
  useScrapingSettings: () => ({ data: state.scraping }),
  useUpdateScrapingSettings: () => ({
    mutateAsync: mockUpdateScrapingMutateAsync,
    isPending: state.updateScrapingIsPending,
  }),
};

/** The real @ajh/ui, overriding only useNotification. */
export async function uiMock(
  importOriginal: () => Promise<typeof AjhUi>
): Promise<Record<string, unknown>> {
  return { ...(await importOriginal()), useNotification: () => mockNotify };
}

/** Reset the per-test state after every test; call once at module top level. */
export function installResetHooks() {
  afterEach(() => {
    state.keyState = {};
    state.setIsPending = false;
    state.removeIsPending = false;
    state.updateScrapingIsPending = false;
    state.scraping = { apifyLinkedinEnabled: false, apifyLinkedinActorId: undefined };
    vi.clearAllMocks();
  });
}

export function getPasswordInputs(container: HTMLElement) {
  return Array.from(container.querySelectorAll('input[type="password"]'));
}

export function firstPasswordInput(container: HTMLElement) {
  const input = getPasswordInputs(container)[0];
  if (!input) throw new Error('No password input found');
  return input;
}

export function firstSaveButton() {
  const button = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.save/i })[0];
  if (!button) throw new Error('No Save button found');
  return button;
}

export function firstRemoveButton() {
  const button = screen.getAllByRole('button', { name: /settings\.aggregatorKeys\.remove/i })[0];
  if (!button) throw new Error('No Remove button found');
  return button;
}

/** The "Remove" confirm button inside the open confirm modal. */
export function modalConfirmButton() {
  const confirm = Array.from(screen.getByRole('dialog').querySelectorAll('button')).find((b) =>
    /settings\.aggregatorKeys\.remove/i.test(b.textContent ?? '')
  );
  if (!confirm) throw new Error('Confirm button not found in modal');
  return confirm;
}
