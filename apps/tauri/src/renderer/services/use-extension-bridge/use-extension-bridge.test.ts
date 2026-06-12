import { describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { createMockClient, makeQueryClient, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import { useExtensionBridgeStatus, useRegenerateExtensionToken } from './use-extension-bridge';

const MOCK_STATUS = { port: 9712, connected: true, token: 'tok-abc123' };

describe('useExtensionBridgeStatus', () => {
  it('returns the status payload from the client', async () => {
    const client = createMockClient({
      'extensionBridge.status': vi.fn().mockResolvedValue(MOCK_STATUS),
    });
    const { result } = renderHookWithClient(() => useExtensionBridgeStatus(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual({ port: 9712, connected: true, token: 'tok-abc123' });
  });

  it('exposes port=null and connected=false when the bridge has not bound', async () => {
    const offline = { port: null, connected: false, token: '' };
    const client = createMockClient({
      'extensionBridge.status': vi.fn().mockResolvedValue(offline),
    });
    const { result } = renderHookWithClient(() => useExtensionBridgeStatus(), { client });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(result.current.data).toEqual(offline);
  });
});

describe('useRegenerateExtensionToken', () => {
  it('invalidates the status query after a successful regeneration', async () => {
    const regenerateToken = vi.fn().mockResolvedValue({ token: 'tok-new999' });
    const client = createMockClient({
      'extensionBridge.status': vi.fn().mockResolvedValue(MOCK_STATUS),
      'extensionBridge.regenerateToken': regenerateToken,
    });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useRegenerateExtensionToken(), {
      client,
      queryClient,
    });

    await act(async () => {
      await result.current.mutateAsync();
    });

    // The onSuccess handler must have called invalidateQueries for the status key.
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({
        queryKey: keys.extensionBridge.status,
      })
    );
  });

  it('calls the client regenerateToken method exactly once per mutate', async () => {
    const regenerateToken = vi.fn().mockResolvedValue({ token: 'tok-xyz' });
    const client = createMockClient({
      'extensionBridge.regenerateToken': regenerateToken,
    });
    const { result } = renderHookWithClient(() => useRegenerateExtensionToken(), { client });

    await act(async () => {
      await result.current.mutateAsync();
    });

    expect(regenerateToken).toHaveBeenCalledTimes(1);
  });

  it('surfaces errors from the client without swallowing them', async () => {
    const error = new Error('rotation failed');
    const client = createMockClient({
      'extensionBridge.regenerateToken': vi.fn().mockRejectedValue(error),
    });
    const { result } = renderHookWithClient(() => useRegenerateExtensionToken(), { client });

    await expect(
      act(async () => {
        await result.current.mutateAsync();
      })
    ).rejects.toThrow('rotation failed');
  });
});
