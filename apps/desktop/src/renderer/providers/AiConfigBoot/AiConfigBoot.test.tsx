/**
 * AiConfigBoot — boot-effect tests (review #16 MEDIUM: no companion test existed
 * though every sibling provider has one, and the seed logic here is exactly
 * what's under scrutiny).
 *
 * Covers:
 *  (a) seeds once and only once — the `seededRef` guard survives a second
 *      hydration-finished callback (the corner the ref exists to protect).
 *  (b) waits for `persist.hasHydrated()` before seeding — does NOT seed
 *      pre-hydration; seeds once `onFinishHydration` fires.
 *  (c) skips seeding when the persisted `aiProviderConfig` is empty
 *      (fresh install — no activeProvider, no provider entries).
 *  (d) invalidates `keys.ai.activeConfig` after a successful seed.
 *
 * `@/store/preferences-store` is mocked (mirrors `use-privacy.test.ts`'s
 * pattern) so hydration timing/config are fully test-controlled rather than
 * depending on real Zustand+localStorage rehydration timing. `vi.hoisted()`
 * holds the mutable mock state since `vi.mock` factories are hoisted above
 * plain top-level `let` declarations.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';

import { createMockClient } from '@/lib/mock-client';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { keys, queryClient } from '@/services/query-client';

import { AiConfigBoot } from './AiConfigBoot';

// ── preferences-store mock — full control over hydration timing + config ──────

const mockPersist = vi.hoisted(() => {
  const state = {
    hasHydrated: true,
    finishHydrationCb: undefined as (() => void) | undefined,
    aiProviderConfig: undefined as
      { activeProvider?: string; providers: Record<string, unknown> } | undefined,
  };
  return {
    state,
    onFinishHydration: vi.fn((cb: () => void) => {
      state.finishHydrationCb = cb;
      return () => {
        state.finishHydrationCb = undefined;
      };
    }),
  };
});

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: {
    getState: () => ({ aiProviderConfig: mockPersist.state.aiProviderConfig }),
    persist: {
      hasHydrated: () => mockPersist.state.hasHydrated,
      onFinishHydration: mockPersist.onFinishHydration,
    },
  },
}));

// ── helpers ─────────────────────────────────────────────────────────────────

function renderBoot(seedActiveConfig = vi.fn().mockResolvedValue({ seeded: true })) {
  const client = createMockClient({ ai: { seedActiveConfig } });
  const utils = render(
    <AppClientProvider client={client}>
      <AiConfigBoot />
    </AppClientProvider>
  );
  return { ...utils, seedActiveConfig };
}

beforeEach(() => {
  mockPersist.state.hasHydrated = true;
  mockPersist.state.finishHydrationCb = undefined;
  mockPersist.state.aiProviderConfig = {
    activeProvider: 'openai',
    providers: { openai: { model: 'gpt-4o' } },
  };
  queryClient.clear();
});

afterEach(() => {
  vi.clearAllMocks();
});

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AiConfigBoot — hydration gating', () => {
  it('does NOT seed before hydration completes', () => {
    mockPersist.state.hasHydrated = false;
    const { seedActiveConfig } = renderBoot();

    expect(seedActiveConfig).not.toHaveBeenCalled();
    expect(mockPersist.onFinishHydration).toHaveBeenCalledOnce();
  });

  it('seeds once onFinishHydration fires', async () => {
    mockPersist.state.hasHydrated = false;
    const { seedActiveConfig } = renderBoot();

    expect(seedActiveConfig).not.toHaveBeenCalled();
    mockPersist.state.finishHydrationCb?.();

    await waitFor(() => expect(seedActiveConfig).toHaveBeenCalledOnce());
  });

  it('seeds immediately when already hydrated on mount', async () => {
    mockPersist.state.hasHydrated = true;
    const { seedActiveConfig } = renderBoot();

    await waitFor(() => expect(seedActiveConfig).toHaveBeenCalledOnce());
  });
});

describe('AiConfigBoot — seededRef guard', () => {
  it('does not seed twice even if the hydration callback fires again', async () => {
    mockPersist.state.hasHydrated = false;
    const { seedActiveConfig } = renderBoot();

    mockPersist.state.finishHydrationCb?.();
    await waitFor(() => expect(seedActiveConfig).toHaveBeenCalledOnce());

    // Simulate a second hydration-finished notification (e.g. a re-rehydrate).
    mockPersist.state.finishHydrationCb?.();
    await new Promise((r) => setTimeout(r, 0));

    expect(seedActiveConfig).toHaveBeenCalledOnce();
  });
});

describe('AiConfigBoot — fresh install skip', () => {
  it('skips seeding when aiProviderConfig has no activeProvider and no provider entries', async () => {
    mockPersist.state.aiProviderConfig = undefined;
    const { seedActiveConfig } = renderBoot();

    await new Promise((r) => setTimeout(r, 0));
    expect(seedActiveConfig).not.toHaveBeenCalled();
  });
});

describe('AiConfigBoot — cache invalidation', () => {
  it('invalidates keys.ai.activeConfig after a successful seed', async () => {
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    renderBoot();

    await waitFor(() =>
      expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: keys.ai.activeConfig })
    );
  });
});
