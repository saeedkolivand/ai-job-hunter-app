/**
 * usePostingActions — interaction tracking, open / save / copy-link / view handlers, pending.
 *
 * Strategy: renderHook in isolation; service hooks stubbed in ./actions-harness;
 * navigator.clipboard replaced with a controlled spy (jsdom has none).
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import {
  mockNavigate,
  mockOpenExternalAsync,
  mockPersistJobAsync,
  mockSaveFromPostingAsync,
  notifyError,
  notifySuccess,
  resetActions,
  run,
  saveState,
  setup,
  withInteractions,
  withMessage,
} from './actions-harness';

beforeEach(resetActions);

describe('usePostingActions — initial interactionTypes', () => {
  it('has() returns false for all types when interactions is undefined', () => {
    const result = setup();
    expect(result.current.has('viewed')).toBe(false);
    expect(result.current.has('opened')).toBe(false);
    expect(result.current.has('bookmarked')).toBe(false);
  });

  it('has() returns true for types already in posting.interactions', () => {
    const result = setup(withInteractions('viewed', 'bookmarked'));
    expect(result.current.has('viewed')).toBe(true);
    expect(result.current.has('bookmarked')).toBe(true);
    expect(result.current.has('opened')).toBe(false);
  });

  it('saved derives from bookmarked interaction in initial state', () => {
    expect(setup(withInteractions('bookmarked')).current.saved).toBe(true);
  });
});

describe('usePostingActions — handleOpen', () => {
  it('calls openExternal.mutateAsync with the posting url', async () => {
    await run(setup(), (a) => a.handleOpen());
    expect(mockOpenExternalAsync).toHaveBeenCalledWith('https://example.com/job/1');
  });

  it('calls persistJob.mutateAsync with interactionType: opened', async () => {
    await run(setup(), (a) => a.handleOpen());
    expect(mockPersistJobAsync).toHaveBeenCalledWith(
      expect.objectContaining({ interactionType: 'opened' })
    );
  });

  it('sets has("opened") to true after handleOpen', async () => {
    const result = setup();
    await run(result, (a) => a.handleOpen());
    expect(result.current.has('opened')).toBe(true);
  });

  it('openExternal.mutateAsync still fires even when persistJob.mutateAsync rejects', async () => {
    mockPersistJobAsync.mockRejectedValueOnce(new Error('network'));

    await run(setup(), (a) => a.handleOpen());

    // openExternal is called synchronously in handleOpen (not awaited inside
    // trackInteraction's try/catch), so it fires regardless of persistJob outcome.
    expect(mockOpenExternalAsync).toHaveBeenCalledWith('https://example.com/job/1');
  });
});

describe('usePostingActions — handleSave', () => {
  it('calls saveFromPosting.mutateAsync with the posting payload', async () => {
    await run(setup(), (a) => a.handleSave());
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

  it('tracks bookmarked interaction so saved becomes true', async () => {
    const result = setup();
    await run(result, (a) => a.handleSave());
    expect(result.current.saved).toBe(true);
  });

  it('notifies success with applications.savedToTracking key', async () => {
    await run(setup(), (a) => a.handleSave());
    expect(notifySuccess).toHaveBeenCalledWith(withMessage('applications.savedToTracking'));
  });

  // Only mark saved after success — the .catch() branch must not touch state.
  it('does NOT mark saved or notify when saveFromPosting rejects', async () => {
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('IPC failure'));

    const result = setup();
    act(() => {
      result.current.handleSave();
    });

    // Wait for the error notify to fire — deterministic signal that the
    // rejected promise settled and the .catch() branch ran.
    await waitFor(() => {
      expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.saveError'));
    });

    expect(result.current.saved).toBe(false);
    expect(notifySuccess).not.toHaveBeenCalled();
  });

  it('marks saved and notifies success AFTER saveFromPosting resolves', async () => {
    const result = setup();
    act(() => {
      result.current.handleSave();
    });

    // Wait for the success notify — deterministic signal the .then() ran.
    await waitFor(() => {
      expect(notifySuccess).toHaveBeenCalledWith(withMessage('applications.savedToTracking'));
    });

    expect(result.current.saved).toBe(true);
  });
});

describe('usePostingActions — handleCopyLink', () => {
  const stubClipboard = (writeText: () => Promise<void> | void) =>
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });

  it('writes the posting url to clipboard and notifies success', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    stubClipboard(writeText);

    await run(setup(), (a) => a.handleCopyLink());

    expect(writeText).toHaveBeenCalledWith('https://example.com/job/1');
    expect(notifySuccess).toHaveBeenCalledWith(withMessage('jobs.copyLink'));
    expect(notifyError).not.toHaveBeenCalled();
  });

  it('notifies error with jobs.copyLinkError key when clipboard.writeText throws', async () => {
    stubClipboard(vi.fn().mockRejectedValue(new Error('DOMException')));

    await run(setup(), (a) => a.handleCopyLink());

    expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.copyLinkError'));
    expect(notifySuccess).not.toHaveBeenCalled();
  });
});

describe('usePostingActions — handleView', () => {
  it('navigates to /applications', async () => {
    await run(setup(), (a) => a.handleView());
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/applications' });
  });
});

// useRowMatchScore is stubbed to return `{ score: undefined }` by default; the
describe('usePostingActions — pending', () => {
  it.each([false, true])('pending mirrors saveFromPosting.isPending (%s)', (isPending) => {
    saveState.isPending = isPending;
    expect(setup().current.pending).toBe(isPending);
  });
});
