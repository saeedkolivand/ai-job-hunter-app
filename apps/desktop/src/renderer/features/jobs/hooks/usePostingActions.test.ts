/**
 * usePostingActions — interaction tracking, action handlers, copy-link error path.
 *
 * Strategy:
 *  - renderHook via @testing-library/react to call the hook in isolation.
 *  - All service hooks are stubbed at module level; spies are reset in beforeEach.
 *  - navigator.clipboard is replaced with a controlled spy (jsdom doesn't implement it).
 *  - Interaction state (`interactionTypes`) is seeded from posting.interactions on init.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── useNotification spy ───────────────────────────────────────────────────────

const notifySuccess = vi.fn();
const notifyError = vi.fn();

vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ success: notifySuccess, error: notifyError }),
}));

// ── router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn().mockResolvedValue(undefined);

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

// ── session store ─────────────────────────────────────────────────────────────

const mockSetApplicationApply = vi.fn();

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel: (s: { setApplicationApply: typeof mockSetApplicationApply }) => unknown) =>
    sel({ setApplicationApply: mockSetApplicationApply }),
}));

// ── service hooks ─────────────────────────────────────────────────────────────

const mockOpenExternalAsync = vi.fn().mockResolvedValue(undefined);
const mockPersistJobAsync = vi.fn().mockResolvedValue(undefined);
const mockSaveFromPostingAsync = vi.fn().mockResolvedValue({ id: 'app-1' });

let mockSaveIsPending = false;

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutateAsync: mockOpenExternalAsync }),
  usePersistJob: () => ({ mutateAsync: mockPersistJobAsync }),
}));

vi.mock('@/services/use-applications', () => ({
  useSaveFromPosting: () => ({
    mutateAsync: mockSaveFromPostingAsync,
    get isPending() {
      return mockSaveIsPending;
    },
  }),
}));

// ── useRowMatchScore — vi.fn() so individual tests can override via mockReturnValueOnce ──

const mockUseRowMatchScore = vi
  .fn()
  .mockReturnValue({ score: undefined, pending: false, hasResume: false });

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: (...args: unknown[]) => mockUseRowMatchScore(...args),
}));

// ── scoreToLevel ──────────────────────────────────────────────────────────────

vi.mock('@/lib/match-level', () => ({
  scoreToLevel: (n: number) => (n >= 0.7 ? 'high' : 'medium'),
}));

// ── component under test ──────────────────────────────────────────────────────

import type { Posting } from '../types';
import { usePostingActions } from './usePostingActions';

// ── fixtures ──────────────────────────────────────────────────────────────────

function makePosting(overrides: Partial<Posting> = {}): Posting {
  return {
    id: 'post-1',
    source: 'linkedin',
    externalId: 'ext-1',
    url: 'https://example.com/job/1',
    title: 'Software Engineer',
    company: 'Acme',
    location: 'Berlin',
    description: 'Great role requiring Rust skills.',
    capturedAt: 1_700_000_000_000,
    ...overrides,
  };
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockOpenExternalAsync.mockClear();
  mockPersistJobAsync.mockClear();
  mockSaveFromPostingAsync.mockClear();
  mockSaveFromPostingAsync.mockResolvedValue({ id: 'app-1' });
  mockNavigate.mockClear();
  mockSetApplicationApply.mockClear();
  notifySuccess.mockClear();
  notifyError.mockClear();
  mockSaveIsPending = false;
  mockUseRowMatchScore.mockReturnValue({ score: undefined, pending: false, hasResume: false });
});

// ─────────────────────────────────────────────────────────────────────────────
// Initial state seeded from posting.interactions
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — initial interactionTypes', () => {
  it('has() returns false for all types when interactions is undefined', () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    expect(result.current.has('viewed')).toBe(false);
    expect(result.current.has('opened')).toBe(false);
    expect(result.current.has('bookmarked')).toBe(false);
  });

  it('has() returns true for types already in posting.interactions', () => {
    const posting = makePosting({
      interactions: [
        {
          interactionType: 'viewed',
          jobId: 'post-1',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
        {
          interactionType: 'bookmarked',
          jobId: 'post-1',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
      ],
    });
    const { result } = renderHook(() => usePostingActions(posting));
    expect(result.current.has('viewed')).toBe(true);
    expect(result.current.has('bookmarked')).toBe(true);
    expect(result.current.has('opened')).toBe(false);
  });

  it('saved derives from bookmarked interaction in initial state', () => {
    const posting = makePosting({
      interactions: [
        {
          interactionType: 'bookmarked',
          jobId: 'post-1',
          timestamp: 0,
          title: 'T',
          company: 'C',
          url: 'u',
          source: 's',
        },
      ],
    });
    const { result } = renderHook(() => usePostingActions(posting));
    expect(result.current.saved).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleOpen
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleOpen', () => {
  it('calls openExternal.mutateAsync with the posting url', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleOpen();
    });
    expect(mockOpenExternalAsync).toHaveBeenCalledWith('https://example.com/job/1');
  });

  it('calls persistJob.mutateAsync with interactionType: opened', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleOpen();
    });
    expect(mockPersistJobAsync).toHaveBeenCalledWith(
      expect.objectContaining({ interactionType: 'opened' })
    );
  });

  it('sets has("opened") to true after handleOpen', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleOpen();
    });
    expect(result.current.has('opened')).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleSave
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleSave', () => {
  it('calls saveFromPosting.mutateAsync with the posting payload', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleSave();
    });
    expect(mockSaveFromPostingAsync).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://example.com/job/1',
        board: 'linkedin',
        company: 'Acme',
        title: 'Software Engineer',
        jobDescription: 'Great role requiring Rust skills.',
      })
    );
  });

  it('forwards scraped salary fields to saveFromPosting when present', async () => {
    const posting = makePosting({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' });
    const { result } = renderHook(() => usePostingActions(posting));
    await act(async () => {
      result.current.handleSave();
    });
    expect(mockSaveFromPostingAsync).toHaveBeenCalledWith(
      expect.objectContaining({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' })
    );
  });

  it('tracks bookmarked interaction so saved becomes true', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleSave();
    });
    expect(result.current.saved).toBe(true);
  });

  it('notifies success with applications.savedToTracking key', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleSave();
    });
    expect(notifySuccess).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'applications.savedToTracking' })
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleCopyLink — success + error path
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleCopyLink', () => {
  it('writes the posting url to clipboard and notifies success', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    });

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleCopyLink();
    });

    expect(writeText).toHaveBeenCalledWith('https://example.com/job/1');
    expect(notifySuccess).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.copyLink' })
    );
    expect(notifyError).not.toHaveBeenCalled();
  });

  it('notifies error with jobs.copyLinkError key when clipboard.writeText throws', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockRejectedValue(new Error('DOMException')) },
      configurable: true,
    });

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleCopyLink();
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.copyLinkError' })
    );
    expect(notifySuccess).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleView
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleView', () => {
  it('navigates to /applications', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleView();
    });
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleTailor — success, error branch, applyMatchLevel variants
// ─────────────────────────────────────────────────────────────────────────────

// useRowMatchScore is stubbed to return `{ score: undefined }` by default.
// The two score-presence variants below override via a fresh mock per test.

describe('usePostingActions — handleTailor', () => {
  it('tracks the applied interaction once the save has succeeded', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });
    // trackInteraction('applied') must have been called (it updates local state + calls persistJob).
    expect(mockPersistJobAsync).toHaveBeenCalledWith(
      expect.objectContaining({ interactionType: 'applied' })
    );
  });

  it('does NOT mark applied when saveFromPosting rejects', async () => {
    // `trackInteraction` PERSISTS via persistJobMutation, and the failure paths
    // return without reverting — so firing it up-front left the posting reading
    // Applied for a Tailor that visibly failed.
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('backend down'));

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.tailorError' })
    );
    expect(mockPersistJobAsync).not.toHaveBeenCalled();
    expect(result.current.has('applied')).toBe(false);
  });

  it('does NOT mark applied when saveFromPosting resolves without an id', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({ id: null });

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(mockPersistJobAsync).not.toHaveBeenCalled();
    expect(result.current.has('applied')).toBe(false);
  });

  it('forwards scraped salary fields to saveFromPosting when present', async () => {
    const posting = makePosting({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' });
    const { result } = renderHook(() => usePostingActions(posting));
    await act(async () => {
      await result.current.handleTailor();
    });
    expect(mockSaveFromPostingAsync).toHaveBeenCalledWith(
      expect.objectContaining({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' })
    );
  });

  it('error branch: saveFromPosting resolves { id: null } → notifies tailorError, no navigate, no setApplicationApply', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({ id: null });

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.tailorError' })
    );
    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });

  it('error branch: saveFromPosting resolves {} (missing id) → same guard fires', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({});

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.tailorError' })
    );
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });

  it('success branch: setApplicationApply called with applyMatchLevel=null when no score', async () => {
    // useRowMatchScore returns { score: undefined } (the module-level default).
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(mockSetApplicationApply).toHaveBeenCalledTimes(1);
    const callArg = mockSetApplicationApply.mock.calls[0]?.[0] as
      Record<string, unknown> | undefined;
    expect(callArg?.applyForId).toBe('app-1');
    expect(callArg?.applyMatchLevel).toBeNull();
    expect(callArg?.applyWizardStep).toBe(0);
    expect(callArg?.applyWizardForm).toBeNull();
    expect(callArg?.applySeedResume).toBeNull();
  });

  it('success branch: setApplicationApply carries applyMatchLevel from scoreToLevel(score.combined)', async () => {
    // mockUseRowMatchScore is the vi.fn() that backs useRowMatchScore.
    // Override for this call only so combined=80 flows into scoreToLevel.
    mockUseRowMatchScore.mockReturnValueOnce({
      score: {
        resumeId: 'r',
        jobId: 'post-1',
        ats: 70,
        semantic: 85,
        combined: 80,
        gaps: [],
        recommendations: [],
      },
      pending: false,
      hasResume: true,
    });

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(mockSetApplicationApply).toHaveBeenCalledTimes(1);
    const callArg = mockSetApplicationApply.mock.calls[0]?.[0] as
      Record<string, unknown> | undefined;
    // scoreToLevel stub: n >= 0.7 → 'high'. combined=80 → 'high'.
    expect(callArg?.applyMatchLevel).toBe('high');
  });

  it('success branch: navigates to /applications/$id with documents tab', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({
        to: '/applications/$id',
        params: { id: 'app-1' },
        search: { tab: 'documents', from: 'jobs' },
      })
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleOpen — error suppression (swallowed catch inside trackInteraction)
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleOpen error suppression', () => {
  it('openExternal.mutateAsync still fires even when persistJob.mutateAsync rejects', async () => {
    mockPersistJobAsync.mockRejectedValueOnce(new Error('network'));

    const { result } = renderHook(() => usePostingActions(makePosting()));
    await act(async () => {
      result.current.handleOpen();
    });

    // openExternal is called synchronously in handleOpen (not awaited inside
    // trackInteraction's try/catch), so it fires regardless of persistJob outcome.
    expect(mockOpenExternalAsync).toHaveBeenCalledWith('https://example.com/job/1');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleTailor — rejected saveFromPosting (fix #5: catch unhandled rejection)
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleTailor rejection', () => {
  it('notifies tailorError and does NOT navigate when saveFromPosting rejects', async () => {
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('IPC failure'));

    const { result } = renderHook(() => usePostingActions(makePosting()));
    // Must not throw — the catch must swallow the rejection cleanly.
    await act(async () => {
      await result.current.handleTailor();
    });

    expect(notifyError).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'jobs.tailorError' })
    );
    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleSave — optimistic-mark fix (fix #6: only mark saved after success)
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — handleSave after-success guard', () => {
  it('does NOT mark saved or notify when saveFromPosting rejects', async () => {
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('IPC failure'));

    const { result } = renderHook(() => usePostingActions(makePosting()));
    act(() => {
      result.current.handleSave();
    });

    // Wait for the error notify to fire — deterministic signal that the
    // rejected promise settled and the .catch() branch ran.
    await waitFor(() => {
      expect(notifyError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'jobs.saveError' })
      );
    });

    expect(result.current.saved).toBe(false);
    expect(notifySuccess).not.toHaveBeenCalled();
  });

  it('marks saved and notifies success AFTER saveFromPosting resolves', async () => {
    const { result } = renderHook(() => usePostingActions(makePosting()));
    act(() => {
      result.current.handleSave();
    });

    // Wait for the success notify — deterministic signal the .then() ran.
    await waitFor(() => {
      expect(notifySuccess).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'applications.savedToTracking' })
      );
    });

    expect(result.current.saved).toBe(true);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// pending mirrors saveFromPosting.isPending
// ─────────────────────────────────────────────────────────────────────────────

describe('usePostingActions — pending', () => {
  it('pending is false when saveFromPosting is not pending', () => {
    mockSaveIsPending = false;
    const { result } = renderHook(() => usePostingActions(makePosting()));
    expect(result.current.pending).toBe(false);
  });

  it('pending is true when saveFromPosting.isPending is true', () => {
    mockSaveIsPending = true;
    const { result } = renderHook(() => usePostingActions(makePosting()));
    expect(result.current.pending).toBe(true);
  });
});
