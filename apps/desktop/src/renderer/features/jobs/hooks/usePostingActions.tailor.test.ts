/**
 * usePostingActions — handleTailor: save-then-track ordering, error branches, apply-wizard seeding.
 *
 * noUncheckedIndexedAccess: all mock.calls[0] accesses are guarded.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { act } from '@testing-library/react';

import {
  type Actions,
  mockNavigate,
  mockPersistJobAsync,
  mockSaveFromPostingAsync,
  mockSetApplicationApply,
  mockUseRowMatchScore,
  notifyError,
  resetActions,
  run,
  salaried,
  setup,
  withMessage,
} from './actions-harness';

beforeEach(resetActions);

describe('usePostingActions — handleTailor', () => {
  const lastApplyArg = () =>
    mockSetApplicationApply.mock.calls[0]?.[0] as Record<string, unknown> | undefined;

  it('tracks the applied interaction once the save has succeeded', async () => {
    await run(setup(), (a) => a.handleTailor());
    // trackInteraction('applied') must have been called (it updates local state + calls persistJob).
    expect(mockPersistJobAsync).toHaveBeenCalledWith(
      expect.objectContaining({ interactionType: 'applied' })
    );
  });

  it('fires persistJob(applied) only AFTER saveFromPosting resolves', async () => {
    // Deferred save: hold the promise open so we can prove the applied
    // interaction is not persisted until the save settles (the #796 guarantee).
    let resolveSave: (value: { id: string }) => void = () => {};
    mockSaveFromPostingAsync.mockReturnValueOnce(
      new Promise<{ id: string }>((resolve) => {
        resolveSave = resolve;
      })
    );

    const result = setup();

    let tailorDone: Promise<void> = Promise.resolve();
    act(() => {
      tailorDone = result.current.handleTailor();
    });

    // Save is in flight but unresolved — applied must NOT be tracked yet.
    expect(mockSaveFromPostingAsync).toHaveBeenCalledTimes(1);
    expect(mockPersistJobAsync).not.toHaveBeenCalled();

    // Resolve the save; only now may the applied interaction persist.
    await act(async () => {
      resolveSave({ id: 'app-1' });
      await tailorDone;
    });

    expect(mockPersistJobAsync).toHaveBeenCalledWith(
      expect.objectContaining({ interactionType: 'applied' })
    );
  });

  it('does NOT mark applied when saveFromPosting rejects', async () => {
    // `trackInteraction` PERSISTS via persistJobMutation, and the failure paths
    // return without reverting — so firing it up-front left the posting reading
    // Applied for a Tailor that visibly failed.
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('backend down'));

    const result = setup();
    await run(result, (a) => a.handleTailor());

    expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.tailorError'));
    expect(mockPersistJobAsync).not.toHaveBeenCalled();
    expect(result.current.has('applied')).toBe(false);
  });

  it('does NOT mark applied when saveFromPosting resolves without an id', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({ id: null });

    const result = setup();
    await run(result, (a) => a.handleTailor());

    expect(mockPersistJobAsync).not.toHaveBeenCalled();
    expect(result.current.has('applied')).toBe(false);
  });

  it.each([
    ['handleSave', (a: Actions) => a.handleSave()],
    ['handleTailor', (a: Actions) => a.handleTailor()],
  ])('%s forwards scraped salary fields to saveFromPosting when present', async (_name, call) => {
    await run(setup(salaried()), call);
    expect(mockSaveFromPostingAsync).toHaveBeenCalledWith(
      expect.objectContaining({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' })
    );
  });

  it('error branch: saveFromPosting resolves { id: null } → notifies tailorError, no navigate, no setApplicationApply', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({ id: null });

    await run(setup(), (a) => a.handleTailor());

    expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.tailorError'));
    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });

  it('error branch: saveFromPosting resolves {} (missing id) → same guard fires', async () => {
    mockSaveFromPostingAsync.mockResolvedValueOnce({});

    await run(setup(), (a) => a.handleTailor());

    expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.tailorError'));
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });

  it('success branch: setApplicationApply called with applyMatchLevel=null when no score', async () => {
    // useRowMatchScore returns { score: undefined } (the default).
    await run(setup(), (a) => a.handleTailor());

    expect(mockSetApplicationApply).toHaveBeenCalledTimes(1);
    const callArg = lastApplyArg();
    expect(callArg?.applyForId).toBe('app-1');
    expect(callArg?.applyMatchLevel).toBeNull();
    expect(callArg?.applyWizardStep).toBe(0);
    expect(callArg?.applyWizardForm).toBeNull();
    expect(callArg?.applySeedResume).toBeNull();
  });

  it('success branch: setApplicationApply carries applyMatchLevel from scoreToLevel(score.combined)', async () => {
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

    await run(setup(), (a) => a.handleTailor());

    expect(mockSetApplicationApply).toHaveBeenCalledTimes(1);
    // scoreToLevel stub: n >= 0.7 → 'high'. combined=80 → 'high'.
    expect(lastApplyArg()?.applyMatchLevel).toBe('high');
  });

  it('success branch: navigates to /applications/$id with documents tab', async () => {
    await run(setup(), (a) => a.handleTailor());

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({
        to: '/applications/$id',
        params: { id: 'app-1' },
        search: { tab: 'documents', from: 'jobs' },
      })
    );
  });

  // Catch the unhandled rejection: the hook must swallow it cleanly.
  it('notifies tailorError and does NOT navigate when saveFromPosting rejects', async () => {
    mockSaveFromPostingAsync.mockRejectedValueOnce(new Error('IPC failure'));

    await run(setup(), (a) => a.handleTailor());

    expect(notifyError).toHaveBeenCalledWith(withMessage('jobs.tailorError'));
    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockSetApplicationApply).not.toHaveBeenCalled();
  });
});
