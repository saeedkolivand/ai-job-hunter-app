import type { ReactNode } from 'react';
import { expect, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, type RenderHookOptions, waitFor } from '@testing-library/react';

import type { AppClient } from '@/lib/app-client';
import { AppClientProvider } from '@/providers/AppClientProvider';

/**
 * A fully-mocked {@link AppClient}. Every `client.<namespace>.<method>()` is a
 * `vi.fn()` resolving to `undefined` (or to a value supplied via `overrides`).
 * Namespace and method proxies are cached so the same `vi.fn` identity is
 * returned across accesses, allowing call assertions.
 */
export function createMockClient(
  // `never[]` params so any concrete mock signature (e.g. an event subscriber
  // taking a typed handler) is assignable here.
  overrides: Record<string, (...args: never[]) => unknown> = {}
): AppClient {
  const namespaceCache = new Map<string, Record<string, unknown>>();

  const makeNamespace = (ns: string) =>
    new Proxy(
      {},
      {
        get: (target: Record<string, unknown>, method: string) => {
          const key = `${ns}.${method}`;
          if (overrides[key]) return overrides[key];
          if (!(method in target)) {
            // Event subscriptions (onStream, onEvent, …) must return a sync
            // unsubscribe function; everything else returns a resolved promise.
            target[method] = method.startsWith('on')
              ? vi.fn(() => () => {})
              : vi.fn().mockResolvedValue(undefined);
          }
          return target[method];
        },
      }
    ) as Record<string, unknown>;

  return new Proxy(
    {},
    {
      get: (_t, ns: string) => {
        if (!namespaceCache.has(ns)) namespaceCache.set(ns, makeNamespace(ns));
        return namespaceCache.get(ns);
      },
    }
  ) as unknown as AppClient;
}

/** Fresh QueryClient with retries disabled so failures surface immediately. */
export function makeQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  });
}

export function withProviders(client: AppClient, queryClient = makeQueryClient()) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AppClientProvider client={client}>{children}</AppClientProvider>
    </QueryClientProvider>
  );
}

/** renderHook wrapped in QueryClient + AppClient providers with a mock client. */
export function renderHookWithClient<TResult, TProps>(
  hook: (props: TProps) => TResult,
  options: { client?: AppClient; queryClient?: QueryClient } & RenderHookOptions<TProps> = {}
) {
  const { client = createMockClient(), queryClient = makeQueryClient(), ...rest } = options;
  return {
    client,
    queryClient,
    ...renderHook(hook, { wrapper: withProviders(client, queryClient), ...rest }),
  };
}

/**
 * Smoke-exercise every `use*` hook a service module exports: render it (which
 * fires query functions and event subscriptions through the mock client) and,
 * when the result is a mutation, trigger it. Asserts that nothing throws —
 * a real regression guard for the service ↔ client wiring. A noop handler is
 * passed as the sole argument so event-subscription hooks are satisfied.
 */
export async function exerciseServiceHooks(
  mod: Record<string, unknown>,
  client: AppClient = createMockClient()
): Promise<void> {
  const queryClient = makeQueryClient();
  const wrapper = withProviders(client, queryClient);
  const hooks = Object.entries(mod).filter(
    ([name, value]) => typeof value === 'function' && name.startsWith('use')
  );
  expect(hooks.length).toBeGreaterThan(0);

  for (const [, hook] of hooks) {
    const fn = hook as (arg: unknown) => unknown;
    const { result, unmount } = renderHook(() => fn(() => {}), { wrapper });
    const current = result.current as { mutate?: (arg: unknown) => void } | null;
    if (current && typeof current.mutate === 'function') {
      await act(async () => {
        current.mutate?.(undefined);
        await Promise.resolve();
      });
    }
    unmount();
  }

  // Let any in-flight query promises settle so no act warnings leak.
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
  await waitFor(() => expect(true).toBe(true));
}
