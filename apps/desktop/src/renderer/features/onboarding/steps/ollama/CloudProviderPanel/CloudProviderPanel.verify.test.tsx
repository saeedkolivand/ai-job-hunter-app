/**
 * CloudProviderPanel — verification-path integration test (CodeRabbit #936,
 * Major 1: "a cached catalogue is being used to verify a newly saved key").
 *
 * `CloudProviderPanel.test.tsx` mocks `@/services` wholesale, which means its
 * `useListProviderModels` stub can't prove anything about the REAL hook's
 * cache-fallback behavior — it just returns whatever a test tells it to. This
 * file renders through the REAL `useListProviderModels` (backed by a mock
 * `AppClient`, via `withProviders`/`createMockClient`) so the actual
 * `purpose: 'verify'` wiring in `fetchProviderModelsWithCache` is exercised,
 * not just asserted by reading the source.
 *
 * The scenario: a cache exists from a PRIOR (possibly different) key for this
 * exact provider + base URL — the cache has no credential identity, so it
 * cannot distinguish "key A's list" from "key B's list". A NEW key is entered
 * and its live request fails. The old cache must NOT be offered as if it
 * verified the new key.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { writeModelListCache } from '@/lib/ai-providers/model-list-cache';
import { fetchProviderModelsWithCache } from '@/services';
import { createMockClient, withProviders } from '@/test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// The real CloudProviderPanel calls useNotification (@ajh/ui) — stub only
// that, keep every other primitive (Dropdown, Alert, Button, Input) real.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    useNotification: () => ({
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
      info: vi.fn(),
      open: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

import { CloudProviderPanel } from './index';

afterEach(() => {
  localStorage.clear();
});

describe('CloudProviderPanel — verification does not trust a cache from a different key', () => {
  it('shows the real failure — never the stale cached list — when a NEW key fails to verify', async () => {
    // Seed the cache the way a PRIOR key's successful fetch would have. The
    // cache key is provider + base URL only — no credential identity.
    writeModelListCache('openai', undefined, [{ name: 'gpt-4o-from-old-key' }]);

    const client = createMockClient({
      'ai.hasProviderKey': vi.fn().mockResolvedValue({ has: true }),
      'ai.listProviderModels': vi
        .fn()
        .mockRejectedValue(new Error('invalid or unauthorized API key')),
    });

    render(
      <CloudProviderPanel
        selectedProvider="openai"
        onProviderChange={vi.fn()}
        selectedModel=""
        onModelSelect={vi.fn()}
      />,
      { wrapper: withProviders(client) }
    );

    // The real failure surfaces...
    expect(await screen.findByText('models.cloud.fetchFailed')).toBeInTheDocument();
    // ...and the differently-keyed cached list is never offered as a pick.
    expect(screen.queryByText('gpt-4o-from-old-key')).not.toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'gpt-4o-from-old-key' })).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
  });

  it('a DISPLAY-purpose read of the exact same seeded cache — e.g. the picker — is allowed to fall back to it', async () => {
    // Same seed, same failing live request — proves the failure above is
    // specific to `purpose: 'verify'`, not a global cache outage.
    writeModelListCache('openai', undefined, [{ name: 'gpt-4o-from-old-key' }]);

    const client = createMockClient({
      'ai.listProviderModels': vi
        .fn()
        .mockRejectedValue(new Error('invalid or unauthorized API key')),
    });

    const result = await fetchProviderModelsWithCache(client, 'openai', undefined, {
      allowCacheFallback: true,
    });

    expect(result).toEqual({ models: [{ name: 'gpt-4o-from-old-key' }], cached: true });
  });
});
