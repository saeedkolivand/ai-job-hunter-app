/**
 * useBestMatchActions — View / Save / Apply / Dismiss for a best-match row.
 *
 * Strategy: module-level mocks for @ajh/translations, @ajh/ui (useNotification),
 * @tanstack/react-router (useNavigate), @/store/session-store, and @/services —
 * mirrors usePostingActions.test.ts's established pattern for a hook that pulls
 * in `useApplyToFoundJob` (itself a real, unmocked import here, so this also
 * exercises the lifted Apply hook end-to-end through the same mocked deps).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { Autopilot, AutopilotBestMatch } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const notifyError = vi.fn();
vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ success: vi.fn(), error: notifyError }),
}));

const mockNavigate = vi.fn().mockResolvedValue(undefined);
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

const mockSetAutopilot = vi.fn();
const mockSetApplicationApply = vi.fn();
vi.mock('@/store/session-store', () => ({
  useSessionStore: (
    sel: (s: {
      setAutopilot: typeof mockSetAutopilot;
      setApplicationApply: typeof mockSetApplicationApply;
    }) => unknown
  ) => sel({ setAutopilot: mockSetAutopilot, setApplicationApply: mockSetApplicationApply }),
}));

const mockOpenExternalMutate = vi.fn();
const mockPersistJobMutate = vi.fn();
const mockRemoveInteractionMutate = vi.fn();
const mockInvalidateQueries = vi.fn();
const mockSaveFromPostingMutateAsync = vi.fn().mockResolvedValue({ id: 'app-1' });
let mockAutopilots: Autopilot[] = [];

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({ invalidateQueries: mockInvalidateQueries }),
}));

vi.mock('@/services', () => ({
  keys: { autopilot: { all: ['autopilot'] } },
  useOpenExternal: () => ({ mutate: mockOpenExternalMutate }),
  usePersistJob: () => ({ mutate: mockPersistJobMutate }),
  useRemoveInteraction: () => ({ mutate: mockRemoveInteractionMutate }),
  useAutopilots: () => ({ data: mockAutopilots }),
  useSaveFromPosting: () => ({ mutateAsync: mockSaveFromPostingMutateAsync }),
}));

import { useBestMatchActions } from './use-best-match-actions';

function makeMatch(overrides: Partial<AutopilotBestMatch> = {}): AutopilotBestMatch {
  return {
    key: 'k1',
    title: 'Backend Engineer',
    company: 'Acme',
    url: 'https://example.com/job/1',
    location: 'Berlin',
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [{ autopilotId: 'ap-1', autopilotName: 'Berlin roles', paused: false, foundAt: 0 }],
    ...overrides,
  };
}

function makeAutopilot(overrides: Partial<Autopilot> = {}): Autopilot {
  return {
    _id: 'ap-1',
    name: 'Berlin roles',
    status: 'active',
    target: { boards: ['linkedin'], query: 'engineer', pages: 1 },
    filter: { minMatchScore: 0 },
    schedule: 'daily',
    totalFound: 0,
    totalApplied: 0,
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockAutopilots = [makeAutopilot()];
  mockSaveFromPostingMutateAsync.mockResolvedValue({ id: 'app-1' });
});

describe('useBestMatchActions — handleView', () => {
  it('opens externally and persists a "viewed" interaction with the job url as id', () => {
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch();

    act(() => result.current.handleView(match));

    expect(mockOpenExternalMutate).toHaveBeenCalledWith(match.url);
    expect(mockPersistJobMutate).toHaveBeenCalledTimes(1);
    const [payload] = mockPersistJobMutate.mock.calls[0] as [
      { job: Record<string, unknown>; interactionType: string },
    ];
    expect(payload.interactionType).toBe('viewed');
    expect(payload.job.id).toBe(match.url);
    expect(payload.job.url).toBe(match.url);
  });

  // Checked DELIBERATELY, not by omission (see the hook's doc comment): a
  // view doesn't change compute_best_matches's qualification predicate, so
  // there is nothing for it to invalidate — no second (options) arg at all,
  // i.e. no onSuccess/invalidate callback wired up.
  it('does not attach an onSuccess/invalidate callback to the persistJob mutation', () => {
    const { result } = renderHook(() => useBestMatchActions());
    act(() => result.current.handleView(makeMatch()));

    expect(mockPersistJobMutate.mock.calls[0]).toHaveLength(1);
  });
});

describe('useBestMatchActions — handleSave', () => {
  it('persists a "bookmarked" interaction', () => {
    const { result } = renderHook(() => useBestMatchActions());
    act(() => result.current.handleSave(makeMatch()));

    expect(mockPersistJobMutate).toHaveBeenCalledTimes(1);
    const [payload] = mockPersistJobMutate.mock.calls[0] as [{ interactionType: string }];
    expect(payload.interactionType).toBe('bookmarked');
  });

  // Same deliberate check as handleView above — a bookmark doesn't change
  // qualification either.
  it('does not attach an onSuccess/invalidate callback to the persistJob mutation', () => {
    const { result } = renderHook(() => useBestMatchActions());
    act(() => result.current.handleSave(makeMatch()));

    expect(mockPersistJobMutate.mock.calls[0]).toHaveLength(1);
  });
});

// ── CRITICAL correctness coupling ───────────────────────────────────────────
//
// Rust matches a dismissal by deriving canonical_job_key(record.url,
// record.title, record.company) against every cluster member's own key. If
// this payload is missing url/title/company (or the id), the derived key
// never matches and the row silently never disappears.

describe('useBestMatchActions — handleDismiss payload', () => {
  it('carries id, url, title AND company — the fields canonical_job_key needs to match the row', () => {
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({
      url: 'https://example.com/job/dismiss-me',
      title: 'Staff Engineer',
      company: 'Globex',
    });

    act(() => result.current.handleDismiss(match));

    expect(mockPersistJobMutate).toHaveBeenCalledTimes(1);
    const [payload] = mockPersistJobMutate.mock.calls[0] as [
      { job: Record<string, unknown>; interactionType: string },
    ];
    expect(payload.interactionType).toBe('dismissed');
    expect(payload.job.id).toBe(match.url);
    expect(payload.job.url).toBe(match.url);
    expect(payload.job.title).toBe('Staff Engineer');
    expect(payload.job.company).toBe('Globex');
  });

  it('optimistically hides the row (adds its key to dismissedKeys) before the mutation resolves', () => {
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({ key: 'k-optimistic' });

    expect(result.current.dismissedKeys.has('k-optimistic')).toBe(false);
    act(() => result.current.handleDismiss(match));
    expect(result.current.dismissedKeys.has('k-optimistic')).toBe(true);
  });

  // Regression for the reported bug: an onSuccess invalidation here forces
  // useBestMatches to refetch before the user could ever reach for Undo, and
  // the backend (compute_best_matches) already excludes a dismissed job from
  // that refetch — evicting the row from the cache entirely and making Undo
  // delete a key for a row that no longer exists anywhere to show. See
  // use-best-match-actions.dismiss-undo.test.tsx for the end-to-end version
  // of this same assertion against a REAL QueryClient (this one only proves
  // no onSuccess callback is wired up at all — cheap, but blind to the
  // cache/timing consequence, which is why that second file exists).
  it('does NOT attach an onSuccess/invalidate callback to the dismiss mutation', () => {
    let capturedOnSuccess: (() => void) | undefined;
    mockPersistJobMutate.mockImplementation((_payload, opts?: { onSuccess?: () => void }) => {
      capturedOnSuccess = opts?.onSuccess;
    });
    const { result } = renderHook(() => useBestMatchActions());

    act(() => result.current.handleDismiss(makeMatch()));

    expect(capturedOnSuccess).toBeUndefined();
  });

  it('rolls back the optimistic hide and surfaces a notification on failure', () => {
    mockPersistJobMutate.mockImplementation((_payload, opts?: { onError?: () => void }) => {
      opts?.onError?.();
    });
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({ key: 'k-fail' });

    act(() => result.current.handleDismiss(match));

    expect(result.current.dismissedKeys.has('k-fail')).toBe(false);
    expect(notifyError).toHaveBeenCalledTimes(1);
  });
});

describe('useBestMatchActions — undoDismiss', () => {
  it('removes a key from dismissedKeys and calls removeInteraction with the SAME (url, "dismissed") pair handleDismiss wrote', () => {
    mockPersistJobMutate.mockImplementation(() => {});
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({ key: 'k-undo', url: 'https://example.com/job/undo-me' });

    act(() => result.current.handleDismiss(match));
    expect(result.current.dismissedKeys.has('k-undo')).toBe(true);

    act(() => result.current.undoDismiss(match.key, match.url));
    expect(result.current.dismissedKeys.has('k-undo')).toBe(false);

    expect(mockRemoveInteractionMutate).toHaveBeenCalledTimes(1);
    const [payload] = mockRemoveInteractionMutate.mock.calls[0] as [
      { jobId: string; interactionType: string },
    ];
    expect(payload).toEqual({ jobId: match.url, interactionType: 'dismissed' });
  });

  it('invalidates keys.autopilot.all on a successful removal', () => {
    mockRemoveInteractionMutate.mockImplementation(
      (_payload, opts?: { onSuccess?: () => void }) => {
        opts?.onSuccess?.();
      }
    );
    const { result } = renderHook(() => useBestMatchActions());

    act(() => result.current.undoDismiss('k1', 'https://example.com/job/1'));

    expect(mockInvalidateQueries).toHaveBeenCalledWith({ queryKey: ['autopilot'] });
  });

  it('rolls back the optimistic reveal and notifies on a failed removal', () => {
    mockRemoveInteractionMutate.mockImplementation((_payload, opts?: { onError?: () => void }) => {
      opts?.onError?.();
    });
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({ key: 'k-fail' });

    act(() => result.current.handleDismiss(match));
    act(() => result.current.undoDismiss(match.key, match.url));

    expect(result.current.dismissedKeys.has('k-fail')).toBe(true);
    expect(notifyError).toHaveBeenCalledTimes(1);
  });
});

describe('useBestMatchActions — handleApply', () => {
  it('resolves the autopilot from sources[0].autopilotId and applies through it', async () => {
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({
      sources: [{ autopilotId: 'ap-1', autopilotName: 'Berlin roles', paused: false, foundAt: 0 }],
    });

    await act(async () => {
      result.current.handleApply(match);
      await Promise.resolve();
    });

    expect(mockSaveFromPostingMutateAsync).toHaveBeenCalledWith(
      expect.objectContaining({ jobUrl: match.url, company: match.company, title: match.title })
    );
  });

  it('does nothing when the source autopilot cannot be resolved (e.g. deleted since)', async () => {
    const { result } = renderHook(() => useBestMatchActions());
    const match = makeMatch({
      sources: [{ autopilotId: 'gone', autopilotName: 'Deleted', paused: false, foundAt: 0 }],
    });

    await act(async () => {
      result.current.handleApply(match);
      await Promise.resolve();
    });

    expect(mockSaveFromPostingMutateAsync).not.toHaveBeenCalled();
  });
});
