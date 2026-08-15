import { describe, expect, it } from 'vitest';

import { IPC_CHANNELS } from '../ipc/contracts/index';
import {
  type AppEvents,
  EVENT_CHANNELS,
  isPipelineSectionKey,
  PIPELINE_STAGE_PHASES,
  type PipelineSectionKey,
  type PipelineStageEvent,
  SECTION_KEY_MAX_LENGTH,
} from './index';

/** Flatten every event-channel wire string out of the namespaced registry. */
function flattenEventChannels(): string[] {
  const all: string[] = [];
  for (const namespace of Object.values(EVENT_CHANNELS)) {
    all.push(...(Object.values(namespace) as string[]));
  }
  return all;
}

/** Flatten every request/response wire string out of `IPC_CHANNELS`. */
function flattenIpcChannels(): string[] {
  const all: string[] = [];
  for (const namespace of Object.values(IPC_CHANNELS)) {
    all.push(...(Object.values(namespace) as string[]));
  }
  return all;
}

describe('EVENT_CHANNELS', () => {
  it('exposes every expected event namespace', () => {
    const namespaces = Object.keys(EVENT_CHANNELS).sort();
    expect(namespaces).toEqual([
      'ai',
      'applications',
      'autopilot',
      'boards',
      'extensionBridge',
      'jobs',
      'menu',
      'notifications',
      'pipeline',
      'scrape',
      'system',
      'updater',
    ]);
  });

  it('maps every event channel to a non-empty string', () => {
    for (const namespace of Object.values(EVENT_CHANNELS)) {
      for (const channel of Object.values(namespace)) {
        expect(typeof channel).toBe('string');
        expect((channel as string).length).toBeGreaterThan(0);
      }
    }
  });

  it('has globally unique event-channel identifiers', () => {
    const all = flattenEventChannels();
    expect(new Set(all).size).toBe(all.length);
  });

  it('never collides with any IPC request/response channel', () => {
    // Events and IPC channels share the same Tauri wire namespace, so a name
    // reused as both a command and a push event would be ambiguous. Assert the
    // two flattened sets are disjoint.
    const eventChannels = new Set(flattenEventChannels());
    const ipcChannels = flattenIpcChannels();
    const intersection = ipcChannels.filter((c) => eventChannels.has(c));
    expect(intersection).toEqual([]);
  });

  it('keeps AppEvents and EVENT_CHANNELS in 1:1 sync', () => {
    // `AppEvents` is keyed by wire name -> payload type. Every registry value
    // must have a payload entry and vice-versa, so the registry and the typed
    // payload map can never drift.
    const appEventKeys: Array<keyof AppEvents> = [
      'ai:stream',
      'jobs:event',
      'applications:changed',
      'notifications:changed',
      'notifications:open',
      'notifications:toast',
      'updater:status',
      'menu:navigate',
      'menu:action',
      'autopilot:focus',
      'autopilot:step',
      'pipeline:stage',
      'scrape:progress',
      'scrape:item',
      'boards:login-status',
      // Windows accent-change push (WinRT UISettings::ColorValuesChanged).
      'system:accentChanged',
      'extensionBridge:changed',
    ];
    const eventChannels = flattenEventChannels().sort();
    expect((appEventKeys as string[]).slice().sort()).toEqual(eventChannels);
  });
});

describe('PipelineStageEvent', () => {
  /**
   * Wire-shape lock for `pipeline:stage`. The Rust emitter (Phase 3) and this
   * interface are hand-synced, so the field NAMES are the contract: renaming
   * one silently breaks the renderer with no type error on the Rust side.
   * The `satisfies` annotation catches a required field being added or removed;
   * the key assertion catches a rename or a casing slip.
   */
  it('pins the full camelCase field set', () => {
    const full = {
      runId: 'run-1',
      jobId: 'job-1',
      stage: 'draft',
      phase: 'finish',
      index: 2,
      total: 6,
      attempt: 1,
      sectionKey: 'experience:0',
      ms: 1234,
      issueCount: 3,
      criticalCount: 1,
    } satisfies PipelineStageEvent;

    expect(Object.keys(full).sort()).toEqual(
      [
        'attempt',
        'criticalCount',
        'index',
        'issueCount',
        'jobId',
        'ms',
        'phase',
        'runId',
        'sectionKey',
        'stage',
        'total',
      ].sort()
    );
  });

  /**
   * The five optional fields really are optional: a `start` event has no
   * duration, no issue counts, and (for a non-section stage) no section key.
   * A required-by-accident field would strand the emitter with nothing to send.
   */
  it('accepts a start event with only the required fields', () => {
    const start = {
      runId: 'run-1',
      jobId: 'job-1',
      stage: 'plan',
      phase: 'start',
      index: 0,
      total: 6,
      attempt: 1,
    } satisfies PipelineStageEvent;

    expect(Object.keys(start).sort()).toEqual([
      'attempt',
      'index',
      'jobId',
      'phase',
      'runId',
      'stage',
      'total',
    ]);
  });

  /**
   * The phase vocabulary is closed — three values, no free-form strings.
   * Asserts against the EXPORTED array (the type's own source of truth), not
   * a literal duplicated here, so a phase added to the array without a
   * corresponding update to this expectation fails — the type itself cannot
   * drift from the array since it is derived from it.
   */
  it('pins the phase vocabulary', () => {
    const phases: readonly PipelineStageEvent['phase'][] = PIPELINE_STAGE_PHASES;
    expect(phases).toEqual(['start', 'finish', 'error']);
  });

  /**
   * `sectionKey` is the ONE model-derived field on a channel that claims to be
   * content-free, so its vocabulary is closed exactly like `phase`'s and the
   * guard is the checkable form of that claim. These cases are the contract the
   * Phase-3 Rust emitter must satisfy before anything reaches the wire.
   */
  describe('sectionKey', () => {
    it('accepts every form of the closed grammar', () => {
      for (const key of [
        'summary',
        'skills',
        'projects',
        'education',
        'experience:0',
        'experience:7',
        'experience:255',
      ]) {
        expect(isPipelineSectionKey(key), key).toBe(true);
      }
    });

    it('rejects a free-form label the model invented', () => {
      for (const key of [
        'Work History',
        'experience', // the index is not optional
        'experience:', // ...nor empty
        'experience:256', // ...nor wider than a u8
        'experience:01', // ...nor zero-padded
        'experience:1.5',
        'experience:-1',
        'experience:0; DROP TABLE runs',
        'SUMMARY', // the grammar is lower-case
        '',
      ]) {
        expect(isPipelineSectionKey(key), key).toBe(false);
      }
    });

    it('rejects an over-length value and anything that is not a string', () => {
      expect(isPipelineSectionKey(`experience:${'9'.repeat(10_000)}`)).toBe(false);
      expect(isPipelineSectionKey('summary'.padEnd(SECTION_KEY_MAX_LENGTH + 1, '!'))).toBe(false);
      for (const notAKey of [undefined, null, 3, {}, ['summary']]) {
        expect(isPipelineSectionKey(notAKey)).toBe(false);
      }
    });

    /** The bound must admit the longest LEGAL key, or the guard contradicts the
     *  grammar it is enforcing. */
    it('bounds the wire without excluding a legal key', () => {
      expect('experience:255'.length).toBeLessThanOrEqual(SECTION_KEY_MAX_LENGTH);
    });

    /**
     * The vocabulary is closed at the TYPE level as well, so a Phase-3 renderer
     * consumer gets the same guarantee statically. `@ts-expect-error` is checked
     * by `tsc` (not by vitest's esbuild), and fails if the assignment ever
     * becomes legal — i.e. if `sectionKey` is widened back to `string`.
     */
    it('closes the vocabulary at the type level, not just at runtime', () => {
      // @ts-expect-error — a free-form label the model invented is not a key.
      const invented: PipelineSectionKey = 'Work History';
      expect(isPipelineSectionKey(invented)).toBe(false);
    });
  });
});
