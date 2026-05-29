import { describe, expect, it } from 'vitest';

import { createMockClient } from './mock-client';

describe('createMockClient', () => {
  it('returns a fully-populated client with every namespace', () => {
    const client = createMockClient();
    expect(Object.keys(client).sort()).toContain('system');
    expect(client.ai).toBeDefined();
    expect(client.documents).toBeDefined();
    expect(client.autopilot).toBeDefined();
  });

  it('provides sensible default resolved values', async () => {
    const client = createMockClient();
    await expect(client.system.getProtocolVersion()).resolves.toBe('1.0.0');
    await expect(client.system.getLocale()).resolves.toBe('en');
    await expect(client.jobs.list()).resolves.toEqual([]);
    await expect(client.documents.list()).resolves.toEqual([]);
    await expect(client.credentials.available()).resolves.toBe(false);
    await expect(client.linkedin.getStatus()).resolves.toEqual({ connected: false });
  });

  it('returns sync unsubscribe handles for event subscriptions', () => {
    const client = createMockClient();
    expect(typeof client.ai.onStream(() => {})).toBe('function');
    expect(typeof client.jobs.onEvent(() => {})).toBe('function');
    expect(typeof client.updater.onStatus(() => {})).toBe('function');
  });

  it('shallow-merges namespace overrides', async () => {
    const client = createMockClient({
      system: { getVersion: async () => '9.9.9' },
    });
    await expect(client.system.getVersion()).resolves.toBe('9.9.9');
    // Non-overridden methods on the same namespace remain intact.
    await expect(client.system.getProtocolVersion()).resolves.toBe('1.0.0');
  });
});
