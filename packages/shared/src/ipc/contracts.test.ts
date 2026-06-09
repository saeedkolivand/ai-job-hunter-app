import { describe, expect, it } from 'vitest';

import { IPC_CHANNELS, PROTOCOL_VERSION } from './contracts/index';

describe('IPC_CHANNELS', () => {
  it('exposes every expected namespace', () => {
    const namespaces = Object.keys(IPC_CHANNELS).sort();
    expect(namespaces).toEqual(
      [
        'ai',
        'aiGenerations',
        'autopilot',
        'boards',
        'credentials',
        'data',
        'dialog',
        'documents',
        'geocode',
        'jobPreferences',
        'contactProfile',
        'jobs',
        'linkedin',
        'match',
        'privacy',
        'referrals',
        'resume',
        'scrape',
        'search',
        'shortcuts',
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

describe('PROTOCOL_VERSION', () => {
  it('is a semver-looking string', () => {
    expect(PROTOCOL_VERSION).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
