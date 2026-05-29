import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { PROTOCOL_VERSION } from '@ajh/shared';

import { createMockClient, renderHookWithClient } from '@/test-support';

import {
  useAppVersion,
  useGetPlatform,
  useProtocolVersionCheck,
  useSetLocale,
  useSystemHealth,
} from './use-system';

describe('use-system services', () => {
  it('useSystemHealth queries system.health', async () => {
    const client = createMockClient({ 'system.health': vi.fn().mockResolvedValue({ ok: true }) });
    const { result } = renderHookWithClient(() => useSystemHealth(), { client });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ ok: true });
  });

  it('useAppVersion returns the backend version', async () => {
    const client = createMockClient({ 'system.getVersion': vi.fn().mockResolvedValue('1.2.3') });
    const { result } = renderHookWithClient(() => useAppVersion(), { client });
    await waitFor(() => expect(result.current.data).toBe('1.2.3'));
  });

  it('useGetPlatform queries the platform', async () => {
    const client = createMockClient({ 'system.getPlatform': vi.fn().mockResolvedValue('win32') });
    const { result } = renderHookWithClient(() => useGetPlatform(), { client });
    await waitFor(() => expect(result.current.data).toBe('win32'));
  });

  it('useProtocolVersionCheck flags a mismatch', async () => {
    const client = createMockClient({
      'system.getProtocolVersion': vi.fn().mockResolvedValue('0.0.0-stale'),
    });
    const { result } = renderHookWithClient(() => useProtocolVersionCheck(), { client });
    await waitFor(() => expect(result.current.checked).toBe(true));
    expect(result.current.mismatch).toBe(true);
    expect(result.current.expected).toBe(PROTOCOL_VERSION);
  });

  it('useProtocolVersionCheck reports a match for the current version', async () => {
    const client = createMockClient({
      'system.getProtocolVersion': vi.fn().mockResolvedValue(PROTOCOL_VERSION),
    });
    const { result } = renderHookWithClient(() => useProtocolVersionCheck(), { client });
    await waitFor(() => expect(result.current.checked).toBe(true));
    expect(result.current.mismatch).toBe(false);
  });

  it('useSetLocale mutation calls system.setLocale', async () => {
    const setLocale = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setLocale': setLocale });
    const { result } = renderHookWithClient(() => useSetLocale(), { client });
    result.current.mutate('de');
    await waitFor(() => expect(setLocale).toHaveBeenCalledWith('de'));
  });
});
