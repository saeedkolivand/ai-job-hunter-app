/**
 * useBoardStatuses — per-board connection status hook.
 *
 * Covers:
 *   - empty array is smoke-safe (returns [] results, anyConnected=false)
 *   - linkedin ID routes to the linkedin status key (not the generic board key)
 *   - generic board ID routes to the generic board status key
 *   - anyConnected=true when at least one board reports connected:true
 *   - anyConnected=false when all boards report connected:false
 *   - anyConnected=false when boardIds is undefined (default)
 */
import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { createMockClient, renderHookWithClient } from '@/test-support';

import { useBoardStatuses } from './use-boards';

describe('useBoardStatuses — empty / default', () => {
  it('returns empty results and anyConnected=false for an empty array', () => {
    const client = createMockClient();
    const { result } = renderHookWithClient(() => useBoardStatuses([]), { client });

    expect(result.current.results).toHaveLength(0);
    expect(result.current.anyConnected).toBe(false);
  });

  it('is smoke-safe when boardIds is undefined (uses default [])', () => {
    const client = createMockClient();
    const { result } = renderHookWithClient(() => useBoardStatuses(undefined), { client });

    expect(result.current.results).toHaveLength(0);
    expect(result.current.anyConnected).toBe(false);
  });
});

describe('useBoardStatuses — LinkedIn routing', () => {
  it('calls api.linkedin.getStatus (not api.boards.getStatus) for the linkedin id', async () => {
    const linkedinGetStatus = vi.fn().mockResolvedValue({ connected: false });
    const boardGetStatus = vi.fn().mockResolvedValue({ connected: false });

    const client = createMockClient({
      'linkedin.getStatus': linkedinGetStatus,
      'boards.getStatus': boardGetStatus,
    });

    const { result } = renderHookWithClient(() => useBoardStatuses(['linkedin']), { client });

    await waitFor(() => expect(result.current.results[0]?.isFetching).toBe(false));

    expect(linkedinGetStatus).toHaveBeenCalled();
    expect(boardGetStatus).not.toHaveBeenCalled();
  });

  it('calls api.boards.getStatus for a non-linkedin board id', async () => {
    const linkedinGetStatus = vi.fn().mockResolvedValue({ connected: false });
    const boardGetStatus = vi.fn().mockResolvedValue({ connected: false });

    const client = createMockClient({
      'linkedin.getStatus': linkedinGetStatus,
      'boards.getStatus': boardGetStatus,
    });

    const { result } = renderHookWithClient(() => useBoardStatuses(['indeed']), { client });

    await waitFor(() => expect(result.current.results[0]?.isFetching).toBe(false));

    expect(boardGetStatus).toHaveBeenCalled();
    expect(linkedinGetStatus).not.toHaveBeenCalled();
  });
});

describe('useBoardStatuses — anyConnected', () => {
  it('anyConnected=true when one board reports connected:true', async () => {
    const client = createMockClient({
      'boards.getStatus': vi.fn().mockResolvedValue({ connected: true }),
    });

    const { result } = renderHookWithClient(() => useBoardStatuses(['indeed']), { client });

    await waitFor(() => expect(result.current.results[0]?.isSuccess).toBe(true));

    expect(result.current.anyConnected).toBe(true);
  });

  it('anyConnected=true when only linkedin reports connected:true', async () => {
    const client = createMockClient({
      'linkedin.getStatus': vi.fn().mockResolvedValue({ connected: true }),
      'boards.getStatus': vi.fn().mockResolvedValue({ connected: false }),
    });

    const { result } = renderHookWithClient(() => useBoardStatuses(['linkedin', 'indeed']), {
      client,
    });

    // Wait until anyConnected flips to true — linkedin is connected:true so it must.
    await waitFor(() => expect(result.current.anyConnected).toBe(true));
  });

  it('anyConnected=false when all boards report connected:false', async () => {
    const client = createMockClient({
      'linkedin.getStatus': vi.fn().mockResolvedValue({ connected: false }),
      'boards.getStatus': vi.fn().mockResolvedValue({ connected: false }),
    });

    const { result } = renderHookWithClient(() => useBoardStatuses(['linkedin', 'indeed']), {
      client,
    });

    await waitFor(() => expect(result.current.results.every((r) => r.isSuccess)).toBe(true));

    expect(result.current.anyConnected).toBe(false);
  });

  it('returns one query result per board id', async () => {
    const client = createMockClient({
      'boards.getStatus': vi.fn().mockResolvedValue({ connected: false }),
    });

    const { result } = renderHookWithClient(
      () => useBoardStatuses(['greenhouse', 'indeed', 'lever']),
      { client }
    );

    // Three boards → three query results
    expect(result.current.results).toHaveLength(3);
  });
});
