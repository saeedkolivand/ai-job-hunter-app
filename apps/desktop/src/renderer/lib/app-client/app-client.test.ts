import { describe, expect, it } from 'vitest';

import { createMockClient } from '../mock-client';
import { _registerClient, getClient } from './app-client';

describe('app-client registry', () => {
  it('returns the registered client from getClient()', () => {
    const client = createMockClient();
    _registerClient(client);
    expect(getClient()).toBe(client);
  });

  it('replaces the client on re-registration', () => {
    const a = createMockClient();
    const b = createMockClient();
    _registerClient(a);
    _registerClient(b);
    expect(getClient()).toBe(b);
  });
});
