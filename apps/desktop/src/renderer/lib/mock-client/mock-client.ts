/**
 * createMockClient — factory for a fully-stubbed AppClient.
 *
 * Intended for:
 *   • Vitest / Jest unit tests of renderer components and service hooks.
 *   • Storybook stories that need a client but no backend.
 *   • A future web HTTP adapter: start with stubs, replace one namespace at a
 *     time with real fetch calls until parity is reached.
 *
 * Usage:
 *   const client = createMockClient();
 *   // override individual methods for a specific test:
 *   const client = createMockClient({
 *     system: { health: async () => ({ status: 'ok' }) },
 *   });
 *
 * Every method is a jest/vitest spy-friendly async stub. Provide overrides as a
 * deep-partial — only the methods you care about need to be specified.
 */
import type { HybridSearchResult, ScrapeProgressEvent } from '@ajh/shared';

import type { AppClient } from '../app-client';
import { appNamespaces } from './namespaces/app';
import { bridgeNamespaces } from './namespaces/bridges';
import { coreNamespaces } from './namespaces/core';
import { emptyList, noop } from './namespaces/helpers';
import { createMockReferrals } from './namespaces/referrals';
import { serviceNamespaces } from './namespaces/services';
import { workspaceNamespaces } from './namespaces/workspace';

type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};

// Where the mock scrape namespace stashes its progress emitter. Off-contract
// (ScrapeContract has no emit surface), so a symbol keeps it out of enumeration
// and object-spread merges — tests reach it via `emitScrapeProgress`.
const SCRAPE_PROGRESS_EMITTER = Symbol('scrapeProgressEmitter');

type ScrapeProgressEmitting = {
  [SCRAPE_PROGRESS_EMITTER]?: (event: ScrapeProgressEvent) => void;
};

export function createMockClient(overrides: DeepPartial<AppClient> = {}): AppClient {
  // In-memory scrape-progress fan-out so tests can drive the onProgress path
  // (register a handler, then push events via `emitScrapeProgress`). Scoped per
  // client so each mock starts with no subscribers.
  const scrapeProgressHandlers = new Set<(event: ScrapeProgressEvent) => void>();

  const base: AppClient = {
    ...coreNamespaces(),
    ...workspaceNamespaces(),
    ...bridgeNamespaces(),

    scrape: {
      boards: noop,
      url: noop,
      resolveUrl: async () => null,
      updateDescription: async () => false,
      persistJob: noop,
      removeInteraction: async () => false,
      listPostings: emptyList,
      clearPostings: noop,
      listInteractions: emptyList,
      // NOT `noop` (resolves `undefined`): `usePostingsSearch`'s success
      // handler reads `data.outcome`/`data.hits` unconditionally, so a
      // mock-backed search must resolve a real (if empty) shape or it
      // throws on every call — breaking Playwright and dev-mode alike, the
      // one caller-visible surface every other mock stub here also honours.
      hybridSearch: async (): Promise<HybridSearchResult> => ({
        outcome: 'ok',
        hits: [],
        arms: { lexical: 'skipped', dense: 'skipped', rerank: 'skipped' },
        corpusSize: 0,
      }),
      onProgress: (handler) => {
        scrapeProgressHandlers.add(handler);
        return () => {
          scrapeProgressHandlers.delete(handler);
        };
      },
    },
    data: {
      export: async () => ({ success: false }),
      import: async () => ({ success: false }),
    },

    ...serviceNamespaces(),
    referrals: createMockReferrals(),
    ...appNamespaces(),
  };

  // Shallow-merge overrides at the namespace level.
  for (const ns of Object.keys(overrides) as Array<string & keyof AppClient>) {
    if (overrides[ns]) {
      (base as unknown as Record<string, unknown>)[ns] = {
        ...(base[ns] as object),
        ...overrides[ns],
      };
    }
  }

  // Attach the progress emitter after merging so it survives a `scrape` override
  // (which replaces the namespace object). Fans an event out to every handler
  // currently registered via `scrape.onProgress`.
  (base.scrape as unknown as ScrapeProgressEmitting)[SCRAPE_PROGRESS_EMITTER] = (event) => {
    for (const handler of scrapeProgressHandlers) handler(event);
  };

  return base;
}

/**
 * Push a scrape-progress event to every handler registered via
 * `client.scrape.onProgress` on a mock client (see {@link createMockClient}).
 * Lets renderer tests exercise the scrape-progress path without a backend.
 * No-op on any client that isn't a mock.
 */
export function emitScrapeProgress(client: AppClient, event: ScrapeProgressEvent): void {
  (client.scrape as unknown as ScrapeProgressEmitting)[SCRAPE_PROGRESS_EMITTER]?.(event);
}
