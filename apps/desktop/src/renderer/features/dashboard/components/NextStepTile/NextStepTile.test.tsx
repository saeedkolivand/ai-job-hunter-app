/**
 * The tile's own half of the derivation: turning three service-hook reads into
 * the three signals, and turning the chosen step into a destination.
 *
 * `next-step.test.ts` covers the ordering and counting; what lives HERE is the
 * mapping the lib can't see — above all the cold-boot rule, that a query with
 * no data yet is `'pending'` and not "unmet", so a returning user never gets a
 * flash of "Add your résumé", and its counterpart, that a REJECTED query is
 * `'unavailable'` and not a silent hidden row.
 *
 * Nothing between this component and the backend is stubbed: the real service
 * hooks and the real `useCanUseAI` run against a mock `AppClient` + a real
 * `QueryClient` (the `useCanUseAI.test.tsx` pattern). The previous version of
 * this file mocked `@/services` to `{ data }` literals and `useCanUseAI` to a
 * fixed object, which made two shipped defects unobservable here: the hook
 * naming `addApiKey` before its key query had answered, and a `dismissed`
 * interaction counting as a tracked job.
 *
 * `@ajh/ui` and `lucide-react` are deliberately NOT mocked (same as
 * `AISystemStatus.test.tsx`): the tile's whole output is an `ActionTile`, and a
 * stub would assert against the stub.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { createMockClient } from '@/lib/mock-client';
import { useSessionStore } from '@/store/session-store';
import { withProviders } from '@/test-support';

const { navigate } = vi.hoisted(() => ({ navigate: vi.fn() }));

// Both mocks spread the real module: the un-stubbed service graph below pulls
// in plenty of other consumers of these packages, and replacing them wholesale
// would break modules that have nothing to do with this test.
vi.mock('@ajh/translations', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
  }),
}));

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  useRouter: () => ({ navigate }),
}));

import { NextStepTile } from './index';

type ClientOverrides = NonNullable<Parameters<typeof createMockClient>[0]>;

const embeddingStatus = (total: number) => ({
  active: { provider: 'ollama', model: 'nomic-embed-text' },
  spaces: [],
  documents: { total, indexedInActiveSpace: total, stale: 0 },
  indexing: false,
});

/** A full interaction row — the contract's eight fields, not just the one. */
const interaction = (interactionType: string) => ({
  jobId: `job-${interactionType}`,
  interactionType,
  timestamp: 1_700_000_000_000,
  title: 'Senior Engineer',
  company: 'ACME',
  url: 'https://example.test/job',
  source: 'test',
  location: 'Remote',
});

/**
 * A client where every signal is answered AND met: a cloud provider with a
 * stored key and a model, one document, one `applied` interaction. Each test
 * overrides only the namespace it is about.
 */
function makeClient(overrides: ClientOverrides = {}) {
  return createMockClient({
    ...overrides,
    ai: {
      activeConfig: async () => ({
        activeProvider: 'openai',
        providers: { openai: { model: 'gpt-4o' } },
      }),
      hasProviderKey: async () => ({ has: true }),
      embeddingStatus: async () => embeddingStatus(1),
      ...overrides.ai,
    },
    scrape: {
      listInteractions: async () => [interaction('applied')],
      ...overrides.scrape,
    },
  });
}

function renderTile(overrides: ClientOverrides = {}) {
  return render(<NextStepTile />, { wrapper: withProviders(makeClient(overrides)) });
}

/**
 * Flush everything that CAN settle. After this, a query with no data is one
 * the test deliberately left unresolved — which is what makes "renders
 * nothing" an assertion about pending-handling rather than about being early.
 */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

beforeEach(() => {
  navigate.mockClear();
});

describe('NextStepTile', () => {
  it('renders nothing while a signal query is still unanswered', async () => {
    const embeddingStatusQuery = vi.fn(() => new Promise<never>(() => {}));
    const { container } = renderTile({ ai: { embeddingStatus: embeddingStatusQuery } });

    await waitFor(() => expect(embeddingStatusQuery).toHaveBeenCalled());
    await settle();

    // Cold boot: the documents count never arrives, so the tile must not
    // advise a returning user to add the résumé they already have.
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing while the AI key check is in flight, then the done row once it answers — never the AI step', async () => {
    // The defect this pins: `useCanUseAI` used to read its not-yet-loaded key
    // query as a settled "no key" and answer `addApiKey`, so a fully
    // configured cloud user got a "Connect an AI model" tile on every cold
    // Dashboard mount, for as long as the key check took.
    let resolveKey: ((value: { has: boolean }) => void) | undefined;
    const hasProviderKey = vi.fn(
      () =>
        new Promise<{ has: boolean }>((resolve) => {
          resolveKey = resolve;
        })
    );
    const { container } = renderTile({ ai: { hasProviderKey } });

    // The key query only STARTS once `activeConfig` has resolved (before that
    // the active provider reads as `ollama`, for which the query is
    // disabled) — so this also proves the config read is already settled.
    await waitFor(() => expect(hasProviderKey).toHaveBeenCalled());
    await settle();
    expect(container).toBeEmptyDOMElement();

    expect(resolveKey).toBeDefined();
    await act(async () => {
      resolveKey?.({ has: true });
    });

    expect(await screen.findByTestId(TEST_IDS.dashboard.nextStepDone)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.dashboard.nextStepTile)).not.toBeInTheDocument();
  });

  it('shows a neutral, permanent help row when a signal query REJECTS — never "setup complete", never nothing', async () => {
    // A rejected query leaves `data` undefined for good. Read as "still
    // loading" it hid the tile — the app's one always-present route to help —
    // for the rest of the session.
    renderTile({
      scrape: { listInteractions: () => Promise.reject(new Error('interactions unavailable')) },
    });

    expect(await screen.findByTestId(TEST_IDS.dashboard.nextStepUnavailable)).toBeInTheDocument();
    expect(screen.getByText('dashboard.nextStep.unavailableTitle')).toBeInTheDocument();
    // Neither of the two claims the tile is not entitled to make.
    expect(screen.queryByTestId(TEST_IDS.dashboard.nextStepDone)).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.dashboard.nextStepTile)).not.toBeInTheDocument();

    await userEvent.click(screen.getByText('dashboard.nextStep.help'));
    expect(navigate).toHaveBeenCalledWith({ to: '/support' });
  });

  it('does not count a dismissed posting as a tracked job', async () => {
    // `useInteractions()` unfiltered includes `dismissed` — the opposite of
    // tracking, and excluded from the "total tracked" stat rendered inches
    // away on the same page. A user who has only ever dismissed a posting has
    // not started their pipeline.
    renderTile({ scrape: { listInteractions: async () => [interaction('dismissed')] } });

    expect(await screen.findByTestId(TEST_IDS.dashboard.nextStepTile)).toBeInTheDocument();
    expect(screen.getByText('dashboard.nextStep.job.title')).toBeInTheDocument();
    expect(screen.getByText(/"done":2.*"total":3/)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.dashboard.nextStepDone)).not.toBeInTheDocument();
  });

  it('asks for a résumé first, counting the steps already met', async () => {
    renderTile({ ai: { embeddingStatus: async () => embeddingStatus(0) } });

    expect(await screen.findByTestId(TEST_IDS.dashboard.nextStepTile)).toBeInTheDocument();
    expect(screen.getByText('dashboard.nextStep.resume.title')).toBeInTheDocument();
    expect(screen.getByText(/"done":2.*"total":3/)).toBeInTheDocument();
  });

  it('deep-links the résumé step to the résumé section of Settings', async () => {
    useSessionStore.getState().setSettings({ activeSection: 'general' });
    renderTile({ ai: { embeddingStatus: async () => embeddingStatus(0) } });

    await userEvent.click(await screen.findByText('dashboard.nextStep.resume.title'));

    // Navigating alone lands on whatever section was last active, so the store
    // write is half the destination — assert both.
    expect(useSessionStore.getState().settings.activeSection).toBe('resume');
    expect(navigate).toHaveBeenCalledWith({ to: '/settings' });
  });

  it('describes the AI step with the real block reason, not a generic nudge', async () => {
    // A cloud provider whose key check ANSWERED "no key" — the settled
    // counterpart of the in-flight case above.
    renderTile({ ai: { hasProviderKey: async () => ({ has: false }) } });

    expect(await screen.findByText('dashboard.nextStep.ai.title')).toBeInTheDocument();
    expect(screen.getByText('aiSetup.addApiKey')).toBeInTheDocument();
  });

  it('sends the job step to the job board', async () => {
    renderTile({ scrape: { listInteractions: async () => [] } });

    await userEvent.click(await screen.findByText('dashboard.nextStep.job.title'));
    expect(navigate).toHaveBeenCalledWith({ to: '/jobs' });
  });

  it('collapses to a persistent done row — never disappears — with a route to help', async () => {
    renderTile();

    expect(await screen.findByTestId(TEST_IDS.dashboard.nextStepDone)).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.dashboard.nextStepTile)).not.toBeInTheDocument();

    await userEvent.click(screen.getByText('dashboard.nextStep.help'));
    expect(navigate).toHaveBeenCalledWith({ to: '/support' });
  });
});
