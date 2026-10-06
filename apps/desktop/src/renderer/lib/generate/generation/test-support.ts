import { afterEach, beforeEach, vi } from 'vitest';

import { keys, queryClient } from '@/services/query-client';
import { usePreferencesStore } from '@/store/preferences-store';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';

// Shared fixtures for the `generation/*.test.ts` suites (no `.test.` in the name,
// so vitest does not collect this file).

let streamHandler: ((chunk: unknown) => void) | null = null;

type MockOverrides = NonNullable<Parameters<typeof createMockClient>[0]>;

/** Mock client whose `ai:stream` subscription is captured for `emit`/`done`.
 *  `extra.ai` adds/overrides `ai` stubs (e.g. `researchCompany`); the streaming
 *  stubs must stay identical or `flushUntilStreaming`/`emit`/`done` never see the job. */
export function register(
  extra: {
    ai?: MockOverrides['ai'];
    contactProfile?: MockOverrides['contactProfile'];
  } = {}
) {
  const client = createMockClient({
    ai: {
      generatePipeline: vi.fn().mockResolvedValue({ jobId: 'gen-1' }),
      onStream: vi.fn((h: (chunk: unknown) => void) => {
        streamHandler = h;
        return () => {};
      }),
      ...extra.ai,
    },
    jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    ...(extra.contactProfile ? { contactProfile: extra.contactProfile } : {}),
  });
  _registerClient(client);
  return client;
}

/** `register()` plus a `contactProfile` override — shared by `generateResume`
 *  and `synthesizeResume`'s H test suites. */
export function registerWithContactProfile(contactProfile: MockOverrides['contactProfile']) {
  return register({ contactProfile });
}

export async function flushUntilStreaming() {
  for (let i = 0; i < 6 && !streamHandler; i++) await Promise.resolve();
}

/** Forget the captured subscription, so a second call within one test waits for
 *  its own subscription instead of emitting into the previous client's. */
export function resetStreamHandler() {
  streamHandler = null;
}

// The active provider/model are backend-owned (task #16) and read imperatively via
// the singleton React Query cache; seed it directly. `modelLimits`/`effort` (tuning
// knobs) still come from Zustand `aiProviderConfig` (set via setState).
export function setActive(activeProvider: string, model: string) {
  queryClient.setQueryData(keys.ai.activeConfig, {
    activeProvider,
    model,
    providers: { [activeProvider]: { model } },
  });
}

/** Drive an already-started generation `call` through one streamed chunk to completion. */
export async function streamThrough<T>(call: Promise<T>, delta: string): Promise<T> {
  await flushUntilStreaming();
  emit(delta);
  done();
  return call;
}

export function emit(delta: string) {
  streamHandler?.({ jobId: 'gen-1', delta, done: false });
}
export function done() {
  streamHandler?.({ jobId: 'gen-1', done: true });
}

/** Per-test setup/teardown every generation suite needs; call once at module top level. */
export function installGenerationHooks() {
  beforeEach(() => {
    vi.useFakeTimers();
    streamHandler = null;
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    usePreferencesStore.setState({ aiProviderConfig: undefined, outputTone: 'professional' });
    queryClient.removeQueries({ queryKey: keys.ai.activeConfig });
  });
}
