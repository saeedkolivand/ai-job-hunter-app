import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useDisplayName } from './use-contact-profile';

const original = usePreferencesStore.getState().userName;
afterEach(() => {
  usePreferencesStore.setState({ userName: original });
});

describe('synced contact display name', () => {
  it('shows the received contact identity without writing a local nickname', async () => {
    usePreferencesStore.setState({ userName: '' });
    const client = createMockClient({
      'contactProfile.get': vi.fn().mockResolvedValue({ fullName: 'Ada Lovelace' }),
    });
    const { result, queryClient } = renderHookWithClient(() => useDisplayName(), { client });
    await waitFor(() => expect(result.current).toBe('Ada Lovelace'));
    expect(usePreferencesStore.getState().userName).toBe('');
    await act(() =>
      queryClient.setQueryData(keys.contactProfile.all, { fullName: 'Grace Hopper' })
    );
    await waitFor(() => expect(result.current).toBe('Grace Hopper'));
  });

  it('preserves an explicit per-device nickname', async () => {
    usePreferencesStore.setState({ userName: '  Ada  ' });
    const client = createMockClient({
      'contactProfile.get': vi.fn().mockResolvedValue({ fullName: 'Ada Lovelace' }),
    });
    const { result } = renderHookWithClient(() => useDisplayName(), { client });
    await waitFor(() => expect(result.current).toBe('Ada'));
  });

  it('handles empty and unavailable contact profiles', async () => {
    usePreferencesStore.setState({ userName: ' ' });
    const client = createMockClient({ 'contactProfile.get': vi.fn().mockResolvedValue({}) });
    const { result } = renderHookWithClient(() => useDisplayName(), { client });
    await waitFor(() => expect(result.current).toBe(''));
  });
});
