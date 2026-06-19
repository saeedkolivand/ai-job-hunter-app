/**
 * use-contact-profile service hooks
 *
 * Strategy:
 *  - createMockClient from test-support (proxy-based spy factory).
 *  - renderHookWithClient wraps QueryClient + AppClientProvider.
 *  - Assertions: useSaveContactProfile calls contactProfile.set with the exact
 *    payload and invalidates keys.contactProfile.all on success.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import type { ContactProfile } from '@ajh/shared';

import {
  createMockClient,
  exerciseServiceHooks,
  makeQueryClient,
  renderHookWithClient,
} from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-contact-profile';
import { useSaveContactProfile } from './use-contact-profile';

afterEach(() => vi.restoreAllMocks());

// ── Smoke ─────────────────────────────────────────────────────────────────────

describe('use-contact-profile service hooks smoke', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useSaveContactProfile ─────────────────────────────────────────────────────

describe('useSaveContactProfile', () => {
  const profile: ContactProfile = {
    fullName: 'Jane Doe',
    email: 'jane@example.com',
    linkedin: 'https://linkedin.com/in/janedoe',
  };

  it('calls api.contactProfile.set with the given profile payload', async () => {
    const set = vi.fn().mockResolvedValue({ success: true });
    const client = createMockClient({ 'contactProfile.set': set });

    const { result } = renderHookWithClient(() => useSaveContactProfile(), { client });

    await act(async () => {
      result.current.mutate(profile);
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(set).toHaveBeenCalledWith(profile);
  });

  it('invalidates keys.contactProfile.all on success', async () => {
    const set = vi.fn().mockResolvedValue({ success: true });
    const client = createMockClient({ 'contactProfile.set': set });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useSaveContactProfile(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate(profile);
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(invalidate).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: keys.contactProfile.all })
    );
  });

  it('does NOT invalidate when the IPC call rejects', async () => {
    const set = vi.fn().mockRejectedValue(new Error('write failed'));
    const client = createMockClient({ 'contactProfile.set': set });
    const queryClient = makeQueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');

    const { result } = renderHookWithClient(() => useSaveContactProfile(), {
      client,
      queryClient,
    });

    await act(async () => {
      result.current.mutate(profile);
    });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(invalidate).not.toHaveBeenCalled();
  });
});
