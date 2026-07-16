/**
 * Unit tests for the auto-track (Task #22, Layer A) background decision +
 * orchestration (apps/extension/src/lib/auto-track.ts).
 *
 * Pure/DI logic — no browser or bridge singletons — so every branch (opt-in
 * gate, tracked→auto-apply, already-applied→noop, untracked→prompt) is covered
 * with plain fakes.
 */

import { describe, expect, it, vi } from 'vitest';

import type { ExtensionAppliedCheckResult, ExtensionStatusUpdateResult } from '@ajh/shared';

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

function flowDeps(overrides: Partial<SubmitFlowDeps> = {}): SubmitFlowDeps {
  return {
    autotrackEnabled: vi.fn().mockResolvedValue(true),
    checkApplied: vi.fn().mockResolvedValue({ found: true, status: 'saved' }),
    updateStatusAuto: vi.fn().mockResolvedValue(OK_UPDATE),
    promptImport: vi.fn(),
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
});
