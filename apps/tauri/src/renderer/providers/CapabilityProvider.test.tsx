import { type ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

import { CapabilityProvider, useCapabilities } from './CapabilityProvider';

const health = {
  ai: { ready: true, model: 'llama3' },
  data: { ready: true, sqlite: true, vector: true },
  workers: { active: 1, idle: 2 },
};

function renderCapabilities(client = createMockClient()) {
  const Providers = withProviders(client);
  return renderHook(() => useCapabilities(), {
    wrapper: ({ children }: { children: ReactNode }) => (
      <Providers>
        <CapabilityProvider>{children}</CapabilityProvider>
      </Providers>
    ),
  });
}

describe('CapabilityProvider', () => {
  it('starts uninitialised with safe defaults', () => {
    const client = createMockClient({ 'system.health': vi.fn(() => new Promise(() => {})) });
    const { result } = renderCapabilities(client);
    expect(result.current.initialized).toBe(false);
    expect(result.current.ai.ready).toBe(false);
  });

  it('maps backend health into capabilities', async () => {
    const client = createMockClient({ 'system.health': vi.fn().mockResolvedValue(health) });
    const { result } = renderCapabilities(client);
    await waitFor(() => expect(result.current.initialized).toBe(true));
    expect(result.current.ai).toEqual({ ready: true, model: 'llama3' });
    expect(result.current.data).toEqual({ ready: true, sqlite: true, vector: true });
    expect(result.current.workers).toEqual({ active: 1, idle: 2 });
  });
});
