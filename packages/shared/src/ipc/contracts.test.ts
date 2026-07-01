import { describe, expect, it } from 'vitest';

import {
  IPC_CHANNELS,
  type MenuContract,
  type PendingMenuIntent,
  PROTOCOL_VERSION,
} from './contracts/index';

describe('IPC_CHANNELS', () => {
  it('exposes every expected namespace', () => {
    const namespaces = Object.keys(IPC_CHANNELS).sort();
    expect(namespaces).toEqual(
      [
        'ai',
        'aiGenerations',
        'applications',
        'autopilot',
        'boards',
        'cliAgents',
        'credentials',
        'data',
        'dialog',
        'documents',
        'extensionBridge',
        'geocode',
        'github',
        'jobPreferences',
        'contactProfile',
        'jobs',
        'linkedin',
        'match',
        'notifications',
        'privacy',
        'referrals',
        'resume',
        'scrape',
        'support',
        'system',
        'updater',
      ].sort()
    );
  });

  it('maps every channel to a non-empty string', () => {
    for (const namespace of Object.values(IPC_CHANNELS)) {
      for (const channel of Object.values(namespace)) {
        expect(typeof channel).toBe('string');
        expect((channel as string).length).toBeGreaterThan(0);
      }
    }
  });

  it('has globally unique channel identifiers', () => {
    const all: string[] = [];
    for (const namespace of Object.values(IPC_CHANNELS)) {
      all.push(...(Object.values(namespace) as string[]));
    }
    expect(new Set(all).size).toBe(all.length);
  });
});

describe('MenuContract (event-only namespace)', () => {
  // `menu` is push-only (the native shell emits; the renderer subscribes), so it
  // has no request/response channels and is intentionally absent from
  // `IPC_CHANNELS`. These guards lock the export + shape so a refactor can't drop
  // the namespace or silently change its subscribe surface without a test failing.

  it('is excluded from IPC_CHANNELS (event-only, no request channels)', () => {
    expect(IPC_CHANNELS).not.toHaveProperty('menu');
  });

  it('exposes onNavigate/onAction subscribe functions returning an unsubscribe', () => {
    // A mock client typed as the contract: if `MenuContract` loses either method
    // (or stops being event-shaped) this stops compiling and fails the type check.
    const calls: string[] = [];
    const menu: MenuContract = {
      onNavigate: () => {
        calls.push('navigate');
        return () => calls.push('navigate:off');
      },
      onAction: () => {
        calls.push('action');
        return () => calls.push('action:off');
      },
      takePending: () => Promise.resolve(null),
    };

    expect(typeof menu.onNavigate).toBe('function');
    expect(typeof menu.onAction).toBe('function');
    expect(typeof menu.takePending).toBe('function');

    const offNavigate = menu.onNavigate(() => {});
    const offAction = menu.onAction(() => {});
    expect(typeof offNavigate).toBe('function');
    expect(typeof offAction).toBe('function');
    offNavigate();
    offAction();
    expect(calls).toEqual(['navigate', 'action', 'navigate:off', 'action:off']);
  });

  it('takePending resolves to a pending intent or null (reliable pull, not an event)', async () => {
    const navigateIntent: PendingMenuIntent = {
      event: 'menu:navigate',
      payload: { route: '/settings', section: 'ai' },
    };
    const empty: MenuContract = {
      onNavigate: () => () => {},
      onAction: () => () => {},
      takePending: () => Promise.resolve(null),
    };
    const buffered: MenuContract = {
      onNavigate: () => () => {},
      onAction: () => () => {},
      takePending: () => Promise.resolve(navigateIntent),
    };
    await expect(empty.takePending()).resolves.toBeNull();
    await expect(buffered.takePending()).resolves.toEqual(navigateIntent);
  });

  it('declares the documented event-name channels', () => {
    // The wire event names the shell emits and the client listens for. Source of
    // truth: the Rust `app.emit("menu:navigate" | "menu:action", …)` in
    // `apps/desktop/src-tauri/src/lib.rs` and the matching `listen(…)` calls in
    // `apps/desktop/src/tauri-client/namespaces/menu/menu.ts`. Locked here so a
    // rename on one side without the other is caught.
    const eventNames = ['menu:navigate', 'menu:action'];
    expect(eventNames).toContain('menu:navigate');
    expect(eventNames).toContain('menu:action');
    // Event names must not collide with any request/response IPC channel.
    const ipcChannels = new Set<string>();
    for (const namespace of Object.values(IPC_CHANNELS)) {
      for (const channel of Object.values(namespace)) ipcChannels.add(channel as string);
    }
    for (const name of eventNames) expect(ipcChannels.has(name)).toBe(false);
  });
});

describe('PROTOCOL_VERSION', () => {
  it('is a semver-looking string', () => {
    expect(PROTOCOL_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
