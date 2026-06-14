import { describe, expect, it } from 'vitest';

import { IPC_CHANNELS } from '../ipc/contracts/index';
import { type AppEvents, EVENT_CHANNELS } from './index';

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
      'jobs',
      'menu',
      'notifications',
      'scrape',
      'shortcuts',
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
      'scrape:progress',
      'scrape:item',
      'boards:login-status',
      // no Rust emit — frontend/global-shortcut only.
      'shortcut:command-palette',
      // Windows accent-change push (WinRT UISettings::ColorValuesChanged).
      'system:accentChanged',
    ];
    const eventChannels = flattenEventChannels().sort();
    expect((appEventKeys as string[]).slice().sort()).toEqual(eventChannels);
  });
});
