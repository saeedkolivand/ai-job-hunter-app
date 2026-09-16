/**
 * Unit tests for the auto-track (Task #22, Layer A) background decision +
 * orchestration (apps/extension/src/lib/auto-track.ts).
 *
 * Pure/DI logic — no browser or bridge singletons — so every branch (opt-in
 * gate, tracked→auto-apply, already-applied→noop, untracked→prompt) is covered
 * with plain fakes.
 */

import { describe, expect, it, vi } from 'vitest';

import type {
  ExtensionAnswersSaveResult,
  ExtensionAppliedCheckResult,
  ExtensionStatusUpdateResult,
} from '@ajh/shared';

import {
  decideSubmitAction,
  handleSubmitDetected,
  maybeArmSubmitWatch,
  type SubmitFlowDeps,
} from './auto-track';

const OK_UPDATE: ExtensionStatusUpdateResult = {
  ok: true,
  applicationId: 'app-1',
  status: 'applied',
};

const OK_SAVE: ExtensionAnswersSaveResult = {
  ok: true,
  applicationId: 'app-1',
  saved: 1,
  skipped: 0,
};

function flowDeps(overrides: Partial<SubmitFlowDeps> = {}): SubmitFlowDeps {
  return {
    autotrackEnabled: vi.fn().mockResolvedValue(true),
    checkApplied: vi.fn().mockResolvedValue({ found: true, status: 'saved' }),
    updateStatusAuto: vi.fn().mockResolvedValue(OK_UPDATE),
    promptImport: vi.fn(),
    saveAnswersAuto: vi.fn().mockResolvedValue(OK_SAVE),
    notifyAutoSave: vi.fn(),
    ...overrides,
  };
}

describe('decideSubmitAction', () => {
  const saved: ExtensionAppliedCheckResult = { found: true, status: 'saved' };

  it('opt-in OFF → noop regardless of the applied result', () => {
    expect(decideSubmitAction(false, saved)).toEqual({ kind: 'noop' });
  });

  it('not tracked → promptImport (never auto-create)', () => {
    expect(decideSubmitAction(true, { found: false })).toEqual({ kind: 'promptImport' });
  });

  it('tracked & saved → autoApply', () => {
    expect(decideSubmitAction(true, saved)).toEqual({ kind: 'autoApply' });
  });

  it('tracked & already applied → noop', () => {
    expect(decideSubmitAction(true, { found: true, status: 'applied' })).toEqual({ kind: 'noop' });
  });

  it('tracked but past saved (e.g. interviewing) → noop (never downgrade)', () => {
    expect(decideSubmitAction(true, { found: true, status: 'interviewing' })).toEqual({
      kind: 'noop',
    });
  });
});

describe('handleSubmitDetected', () => {
  it('opt-in OFF → never checks, never writes, never prompts', async () => {
    const deps = flowDeps({ autotrackEnabled: vi.fn().mockResolvedValue(false) });
    await handleSubmitDetected('https://x.co/j', deps);
    expect(deps.checkApplied).not.toHaveBeenCalled();
    expect(deps.updateStatusAuto).not.toHaveBeenCalled();
    expect(deps.promptImport).not.toHaveBeenCalled();
  });

  it('tracked & saved → auto-marks applied (no prompt)', async () => {
    const deps = flowDeps();
    await handleSubmitDetected('https://x.co/j', deps);
    expect(deps.updateStatusAuto).toHaveBeenCalledWith('https://x.co/j');
    expect(deps.promptImport).not.toHaveBeenCalled();
  });

  it('already applied → silent no-op (no write, no prompt)', async () => {
    const deps = flowDeps({
      checkApplied: vi.fn().mockResolvedValue({ found: true, status: 'applied' }),
    });
    await handleSubmitDetected('https://x.co/j', deps);
    expect(deps.updateStatusAuto).not.toHaveBeenCalled();
    expect(deps.promptImport).not.toHaveBeenCalled();
  });

  it('untracked → prompts import (never writes)', async () => {
    const deps = flowDeps({ checkApplied: vi.fn().mockResolvedValue({ found: false }) });
    await handleSubmitDetected('https://x.co/j', deps);
    expect(deps.promptImport).toHaveBeenCalledTimes(1);
    expect(deps.updateStatusAuto).not.toHaveBeenCalled();
  });

  it('is best-effort — a checkApplied failure is swallowed (no prompt, no write)', async () => {
    const deps = flowDeps({ checkApplied: vi.fn().mockRejectedValue(new Error('bridge down')) });
    await expect(handleSubmitDetected('https://x.co/j', deps)).resolves.toBeUndefined();
    expect(deps.updateStatusAuto).not.toHaveBeenCalled();
    expect(deps.promptImport).not.toHaveBeenCalled();
  });

  it('no answers argument → never saves, never notifies (PR4)', async () => {
    const deps = flowDeps();
    await handleSubmitDetected('https://x.co/j', deps);
    expect(deps.saveAnswersAuto).not.toHaveBeenCalled();
    expect(deps.notifyAutoSave).not.toHaveBeenCalled();
  });

  it('an empty captured-answers array → never saves (PR4)', async () => {
    const deps = flowDeps();
    await handleSubmitDetected('https://x.co/j', deps, []);
    expect(deps.saveAnswersAuto).not.toHaveBeenCalled();
  });

  it('captured answers present → saves with auto:true and notifies on success (PR4)', async () => {
    const deps = flowDeps();
    const answers = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    await handleSubmitDetected('https://x.co/j', deps, answers);
    expect(deps.saveAnswersAuto).toHaveBeenCalledWith('https://x.co/j', answers);
    expect(deps.notifyAutoSave).toHaveBeenCalledWith(OK_SAVE);
  });

  it('a desktop refusal (e.g. the opt-in off server-side) degrades silently — no notice (PR4)', async () => {
    const deps = flowDeps({
      saveAnswersAuto: vi.fn().mockResolvedValue({ ok: false, error: 'auto_save_disabled' }),
    });
    const answers = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    await handleSubmitDetected('https://x.co/j', deps, answers);
    expect(deps.notifyAutoSave).not.toHaveBeenCalled();
  });

  it('auto-track opt-in OFF also skips the answer save (nested, PR4)', async () => {
    const deps = flowDeps({ autotrackEnabled: vi.fn().mockResolvedValue(false) });
    const answers = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    await handleSubmitDetected('https://x.co/j', deps, answers);
    expect(deps.saveAnswersAuto).not.toHaveBeenCalled();
  });

  it('a saveAnswersAuto failure is swallowed (best-effort, PR4)', async () => {
    const deps = flowDeps({
      saveAnswersAuto: vi.fn().mockRejectedValue(new Error('bridge down')),
    });
    const answers = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    await expect(handleSubmitDetected('https://x.co/j', deps, answers)).resolves.toBeUndefined();
    expect(deps.notifyAutoSave).not.toHaveBeenCalled();
  });
});

describe('maybeArmSubmitWatch', () => {
  it('opt-in OFF → does NOT inject the watcher', async () => {
    const injectSubmitWatch = vi.fn().mockResolvedValue(undefined);
    await maybeArmSubmitWatch({
      autotrackEnabled: vi.fn().mockResolvedValue(false),
      injectSubmitWatch,
    });
    expect(injectSubmitWatch).not.toHaveBeenCalled();
  });

  it('opt-in ON → injects the watcher', async () => {
    const injectSubmitWatch = vi.fn().mockResolvedValue(undefined);
    await maybeArmSubmitWatch({
      autotrackEnabled: vi.fn().mockResolvedValue(true),
      injectSubmitWatch,
    });
    expect(injectSubmitWatch).toHaveBeenCalledTimes(1);
  });

  it('is best-effort — an injection failure never throws', async () => {
    await expect(
      maybeArmSubmitWatch({
        autotrackEnabled: vi.fn().mockResolvedValue(true),
        injectSubmitWatch: vi.fn().mockRejectedValue(new Error('restricted page')),
      })
    ).resolves.toBeUndefined();
  });

  it('no saveAnswersOnSubmitEnabled dep → arms WITHOUT capture (PR4, unchanged default)', async () => {
    const injectSubmitWatch = vi.fn().mockResolvedValue(undefined);
    await maybeArmSubmitWatch({
      autotrackEnabled: vi.fn().mockResolvedValue(true),
      injectSubmitWatch,
    });
    expect(injectSubmitWatch).toHaveBeenCalledWith(false);
  });

  it('saveAnswersOnSubmitEnabled true → arms WITH capture (PR4)', async () => {
    const injectSubmitWatch = vi.fn().mockResolvedValue(undefined);
    await maybeArmSubmitWatch({
      autotrackEnabled: vi.fn().mockResolvedValue(true),
      injectSubmitWatch,
      saveAnswersOnSubmitEnabled: vi.fn().mockResolvedValue(true),
    });
    expect(injectSubmitWatch).toHaveBeenCalledWith(true);
  });

  it('saveAnswersOnSubmitEnabled failure → still arms, WITHOUT capture (PR4, best-effort)', async () => {
    const injectSubmitWatch = vi.fn().mockResolvedValue(undefined);
    await maybeArmSubmitWatch({
      autotrackEnabled: vi.fn().mockResolvedValue(true),
      injectSubmitWatch,
      saveAnswersOnSubmitEnabled: vi.fn().mockRejectedValue(new Error('bridge down')),
    });
    expect(injectSubmitWatch).toHaveBeenCalledWith(false);
  });
});
