import { expect, type Mock, vi } from 'vitest';
import { act, waitFor } from '@testing-library/react';

import { generateHelpAnswer } from '@/lib/generate';
import { createMockClient, renderHookWithClient } from '@/test-support';

import { useHelpChat } from '../use-help-chat';
import { glanceRecorder } from './test-mocks';

/** `exportDoc` lives in the aiGenerate section — NOT the applications one. */
export const HIT = { id: 'exportDoc', score: 0.9 };
/** `trackJob` is a `support.faq.applicationsQuestions.*` entry. */
export const APPLICATIONS_HIT = { id: 'trackJob', score: 0.9 };
/** `setUpAutopilot` is a `support.faq.autopilotQuestions.*` entry. */
export const AUTOPILOT_HIT = { id: 'setUpAutopilot', score: 0.9 };

export const ANSWER = 'Open the document and click Export.';

/**
 * Two autopilots as the backend returns them: user-typed names, and the rest of
 * the record — including the résumé text the glance must never carry.
 */
const AUTOPILOTS = [
  {
    _id: 'ap1',
    name: 'Berlin React roles',
    status: 'active',
    runStatus: 'completed',
    totalFound: 12,
    resumeText: 'SECRET resume text',
  },
  { _id: 'ap2', name: 'Remote Rust', status: 'paused', totalFound: 0 },
];

/** A `help.search` reply that ranked `results`, with both arms having run. */
export const hybridReply = (...results: Array<{ id: string; score: number }>) => ({
  results,
  mode: 'hybrid',
  arms: { lexical: 'ran', dense: 'ran' },
});

/** A `help.search` stub resolving to {@link hybridReply}. */
export const hybridSearch = (...results: Array<{ id: string; score: number }>): Mock =>
  vi.fn().mockResolvedValue(hybridReply(...results));

/** A `help.search` stub whose first call stays pending until `settle` is invoked. */
export function pendingSearch() {
  const handle: { settle?: (value: unknown) => void; search: Mock<() => Promise<unknown>> } = {
    search: vi.fn<() => Promise<unknown>>(),
  };
  handle.search.mockImplementation(
    () =>
      new Promise((resolve) => {
        handle.settle = resolve;
      })
  );
  return handle;
}

export function client(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  return createMockClient({
    'help.search': hybridSearch(HIT),
    'ai.embeddingStatus': vi
      .fn()
      .mockResolvedValue({ documents: { total: 3, indexedInActiveSpace: 3, stale: 0 } }),
    'scrape.listInteractions': vi.fn().mockResolvedValue([
      { interactionType: 'viewed' },
      { interactionType: 'viewed' },
      // `dismissed` is NOT a tracked type — it must never reach the glance.
      { interactionType: 'dismissed' },
    ]),
    'applications.list': vi
      .fn()
      .mockResolvedValue([
        { id: 'a1', title: 'Senior Engineer', company: 'Acme', status: 'applied', updatedAt: 2 },
      ]),
    'autopilot.list': vi.fn().mockResolvedValue(AUTOPILOTS),
    ...overrides,
  });
}

/** Reset the recorded glance input and the stubbed answer (call in `beforeEach`). */
export function resetChatMocks() {
  glanceRecorder.inputs.length = 0;
  vi.mocked(generateHelpAnswer).mockClear();
  vi.mocked(generateHelpAnswer).mockResolvedValue(ANSWER);
}

/**
 * Render the hook. Nothing is awaited here on purpose: the four lists behind the
 * data glance are fetched inside `send`, not on mount, so there is no
 * "wait for the queries to land" step — see the privacy test.
 *
 * `llama3` has no parseable parameter size, so `detectModelSize` classifies it
 * `small`; `llama3:70b` is the large-tier model in these tests.
 */
export function renderChat(model = 'llama3:70b', overrides = {}) {
  const mock = client(overrides);
  const rendered = renderHookWithClient(() => useHelpChat({ model, canUse: true }), {
    client: mock,
  });
  return { ...rendered, mock };
}

/** Send `question` and flush the run. */
export async function ask(
  result: { current: { send: (q: string) => Promise<boolean> } },
  question: string
) {
  await act(async () => {
    await result.current.send(question);
  });
}

interface SearchRequest {
  queryId: string;
  locale: string;
  query: string;
  entries: Array<{ id: string; title: string; body: string }>;
  limit: number;
}

export const searchArgAt = (search: ReturnType<typeof vi.fn>, index: number) =>
  search.mock.calls[index]?.[0] as SearchRequest;

export const searchArg = (mock: ReturnType<typeof client>) =>
  searchArgAt(mock.help.search as ReturnType<typeof vi.fn>, 0);

export const generateArg = () =>
  vi.mocked(generateHelpAnswer).mock.calls[0]?.[0] as Parameters<typeof generateHelpAnswer>[0];

/** The glance text the hook handed the model for the first answer. */
export const firstGlance = () => generateArg().dataGlance ?? '';

/** The one field these tests read back off {@link glanceRecorder}. */
interface RecordedGlanceInput {
  autopilots?: ReadonlyArray<{ name: string }> | null;
}

/** The autopilot list as the hook passed it, before the prompt renders it. */
export const glanceAutopilotsSent = () =>
  (glanceRecorder.inputs.at(-1) as RecordedGlanceInput | undefined)?.autopilots ?? null;

/** The four reads that make up the data glance. */
export const dataReads = (mock: ReturnType<typeof client>) =>
  [
    mock.ai.embeddingStatus,
    mock.scrape.listInteractions,
    mock.applications.list,
    mock.autopilot.list,
  ] as ReturnType<typeof vi.fn>[];

/** Start `question` without awaiting it, and wait until the hook reports `streaming`. */
export async function startAndWaitStreaming(
  result: { current: { send: (q: string) => Promise<boolean>; streaming: boolean } },
  question: string,
  send = result.current.send
) {
  await act(async () => {
    void send(question);
    await waitFor(() => expect(result.current.streaming).toBe(true));
  });
}
