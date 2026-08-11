import { describe, expect, it } from 'vitest';

import { IPC_CHANNELS } from '../ipc/contracts/index';
import { type AppEvents, EVENT_CHANNELS, type PipelineStageEvent } from './index';

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
      'agent',
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
      'agent:step',
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
      sectionKey: 'experience',
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

  /** The phase vocabulary is closed — three values, no free-form strings. */
  it('pins the phase vocabulary', () => {
    const phases: Array<PipelineStageEvent['phase']> = ['start', 'finish', 'error'];
    expect(phases).toEqual(['start', 'finish', 'error']);
  });
});
