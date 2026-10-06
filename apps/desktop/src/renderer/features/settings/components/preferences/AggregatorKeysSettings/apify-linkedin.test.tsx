/**
 * AggregatorKeysSettings — Apify LinkedIn section.
 *
 *  - toggle fires updateScrapingSettings with enabled=true.
 *  - toggle error uses i18n key, not raw error.
 *  - actor-id Save calls updateScrapingSettings with the typed value.
 *  - actor-id Save is re-entrancy guarded (isPending).
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AggregatorKeysSettings } from './index';
import {
  installResetHooks,
  mockNotify,
  mockUpdateScrapingMutateAsync,
  state,
} from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-support')).translationsMock);
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-support')).uiMock(importOriginal)
);
vi.mock('@/services', async () => (await import('./test-support')).servicesMock);

installResetHooks();

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
    state.updateScrapingIsPending = true;
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
