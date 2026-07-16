import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-email-watch';
import {
  useConnectEmailWatch,
  useDisconnectEmailWatch,
  useEmailWatchCheckNow,
  useEmailWatchStatus,
  useSetEmailWatchEnabled,
} from './use-email-watch';

const DISCONNECTED = { connected: false, enabled: false };
const CONNECTED = {
  connected: true,
  address: 'me@gmail.com',
  enabled: false,
  lastCheckAt: 1_700_000_000_000,
};

describe('use-email-watch services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useEmailWatchStatus', () => {
  it('returns the status payload from the client', async () => {
    const client = createMockClient({
      'emailWatch.status': vi.fn().mockResolvedValue(CONNECTED),
    });
    const { result } = renderHookWithClient(() => useEmailWatchStatus(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(CONNECTED);
  });
});

describe('useConnectEmailWatch', () => {
  it('forwards address + appPassword and seeds the status cache on success', async () => {
    const connect = vi.fn().mockResolvedValue(CONNECTED);
    const client = createMockClient({
      'emailWatch.status': vi.fn().mockResolvedValue(DISCONNECTED),
      'emailWatch.connect': connect,
    });
    const { result, queryClient } = renderHookWithClient(() => useConnectEmailWatch(), { client });

    await act(async () => {
      await result.current.mutateAsync({
        address: 'me@gmail.com',
        appPassword: 'abcd efgh ijkl mnop',
      });
    });

    expect(connect).toHaveBeenCalledWith({
      address: 'me@gmail.com',
      appPassword: 'abcd efgh ijkl mnop',
    });
    await waitFor(() => {
      expect(queryClient.getQueryData(keys.emailWatch.status)).toEqual(CONNECTED);
    });
  });

  it('surfaces a rejection (typed IMAP login failure) without swallowing it', async () => {
    const error = new Error('IMAP LOGIN failed');
    const client = createMockClient({
      'emailWatch.connect': vi.fn().mockRejectedValue(error),
    });
    const { result } = renderHookWithClient(() => useConnectEmailWatch(), { client });

    await expect(
      act(async () => {
        await result.current.mutateAsync({ address: 'me@gmail.com', appPassword: 'bad' });
      })
    ).rejects.toThrow('IMAP LOGIN failed');
  });
});

describe('useDisconnectEmailWatch', () => {
  it('seeds the status cache with the disconnected result', async () => {
    const client = createMockClient({
      'emailWatch.disconnect': vi.fn().mockResolvedValue(DISCONNECTED),
    });
    const { result, queryClient } = renderHookWithClient(() => useDisconnectEmailWatch(), {
      client,
    });

    await act(async () => {
      await result.current.mutateAsync();
    });

    await waitFor(() => {
      expect(queryClient.getQueryData(keys.emailWatch.status)).toEqual(DISCONNECTED);
    });
  });
});

describe('useSetEmailWatchEnabled', () => {
  it('forwards the enabled flag and seeds the returned status', async () => {
    const setEnabled = vi.fn().mockResolvedValue({ ...CONNECTED, enabled: true });
    const client = createMockClient({ 'emailWatch.setEnabled': setEnabled });
    const { result, queryClient } = renderHookWithClient(() => useSetEmailWatchEnabled(), {
      client,
    });

    await act(async () => {
      await result.current.mutateAsync(true);
    });

    expect(setEnabled).toHaveBeenCalledWith(true);
    await waitFor(() => {
      expect(queryClient.getQueryData(keys.emailWatch.status)).toEqual({
        ...CONNECTED,
        enabled: true,
      });
    });
  });
});

describe('useEmailWatchCheckNow', () => {
  it('re-validates the connection and seeds the refreshed status (lastCheckAt bump)', async () => {
    const refreshed = { ...CONNECTED, lastCheckAt: 1_700_000_500_000 };
    const client = createMockClient({
      'emailWatch.checkNow': vi.fn().mockResolvedValue(refreshed),
    });
    const { result, queryClient } = renderHookWithClient(() => useEmailWatchCheckNow(), {
      client,
    });

    await act(async () => {
      await result.current.mutateAsync();
    });

    await waitFor(() => {
      expect(queryClient.getQueryData(keys.emailWatch.status)).toEqual(refreshed);
    });
  });
});
