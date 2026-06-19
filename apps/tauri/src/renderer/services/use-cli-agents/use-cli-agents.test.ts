/**
 * use-cli-agents service hooks
 *
 * Strategy:
 *  - createMockClient from test-support (proxy-based spy factory).
 *  - renderHookWithClient wraps QueryClient + AppClientProvider.
 *  - Assertions: useInstallCliAgent calls install, then redetect, then invalidates
 *    keys.cliAgents.all — the double-IPC sequence is verified.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
} from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-cli-agents';
import { useInstallCliAgent } from './use-cli-agents';

afterEach(() => vi.restoreAllMocks());

// ── Smoke ─────────────────────────────────────────────────────────────────────

describe('use-cli-agents service hooks smoke', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useInstallCliAgent ────────────────────────────────────────────────────────

describe('useInstallCliAgent', () => {
  it('calls api.cliAgents.install with the given options', async () => {
    const install = vi.fn().mockResolvedValue({ success: true, version: '1.0.0' });
    const redetect = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({
      'cliAgents.install': install,
      'cliAgents.redetect': redetect,
    });

    const { result } = renderHookWithClient(() => useInstallCliAgent(), { client });

    await act(async () => {
      result.current.mutate({ commandName: 'claude', args: ['--version'] });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(install).toHaveBeenCalledWith({ commandName: 'claude', args: ['--version'] });
  });

  it('calls api.cliAgents.redetect() after a successful install', async () => {
    const install = vi.fn().mockResolvedValue({ success: true, version: '1.0.0' });
    const redetect = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({
      'cliAgents.install': install,
      'cliAgents.redetect': redetect,
    });

    const { result } = renderHookWithClient(() => useInstallCliAgent(), { client });

    await act(async () => {
      result.current.mutate({ commandName: 'claude', args: [] });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    // redetect busts the Rust-side cache; it must fire after install succeeds.
    expect(redetect).toHaveBeenCalledTimes(1);
  });

  it('invalidates keys.cliAgents.all after a successful install', async () => {
    const install = vi.fn().mockResolvedValue({ success: true, version: '1.0.0' });
    const redetect = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({
      'cliAgents.install': install,
      'cliAgents.redetect': redetect,
    });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useInstallCliAgent(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate({ commandName: 'claude', args: [] });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.cliAgents.all })
    );
  });

  it('redetect is called before invalidateQueries (double-IPC sequence)', async () => {
    // Assert ordering: redetect (busts Rust cache) must precede the React Query
    // invalidation so the re-fetch sees fresh data.
    const callOrder: string[] = [];
    const install = vi.fn().mockResolvedValue({ success: true, version: '1.0.0' });
    const redetect = vi.fn().mockImplementation(async () => {
      callOrder.push('redetect');
    });
    const client = createMockClient({
      'cliAgents.install': install,
      'cliAgents.redetect': redetect,
    });
    const queryClient = makeQueryClient();
    vi.spyOn(queryClient, 'invalidateQueries').mockImplementation(async () => {
      callOrder.push('invalidate');
    });

    const { result } = renderHookWithClient(() => useInstallCliAgent(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate({ commandName: 'claude', args: [] });
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(callOrder).toEqual(['redetect', 'invalidate']);
  });
});
