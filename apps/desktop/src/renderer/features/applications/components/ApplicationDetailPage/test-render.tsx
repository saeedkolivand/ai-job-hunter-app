/**
 * Page-render entry for the ApplicationDetailPage suites. It registers every
 * `vi.mock` the page graph needs (hoisted above the `./index` import below), so
 * a suite must import the page FROM HERE, never from `./index`. The controllable
 * state the mocks read lives in ./test-support.
 */

import { beforeEach, vi } from 'vitest';
import { render } from '@testing-library/react';

import type { Application, StatusEvent } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { ApplicationDetailPage as PageUnderTest } from './index';
import {
  makeApp,
  mockAcceptStatusEventMutate,
  mockImportJobUrlMutate,
  mockNavigate,
  mockNotify,
  mockRejectStatusEventMutate,
  mockRemoveMutateAsync,
  mockSessionState,
  mockSetStatusMutate,
  mockUpdateApplicationMutate,
  mockUseAiGenerations,
  mockUseApplication,
  resetMocks,
  setLoaded,
  state,
} from './test-support';

beforeEach(resetMocks);

/** The page under test — a wrapper, so the (mock-hoisted) import is read at render time. */
export const ApplicationDetailPage = () => <PageUnderTest />;

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

// Everything real except useNotification (no provider in this tree; the
// timeline's accept/reject toasts need a controllable spy).
vi.mock('@ajh/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof AjhUi>()),
  useNotification: () => mockNotify,
}));

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => mockNavigate }));

vi.mock('@/routes/applications.$id', () => ({
  DETAIL_TABS: ['overview', 'timeline', 'brief', 'documents'] as const,
  Route: {
    useParams: () => ({ id: 'app-1' }),
    useSearch: () => ({ tab: state.tab, from: state.from }),
  },
}));

vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector?: (s: typeof mockSessionState) => unknown) =>
    selector ? selector(mockSessionState) : mockSessionState,
}));

vi.mock('@/hooks/use-format-relative-time', () => ({ useFormatRelativeTime: () => () => '' }));

vi.mock('@/features/documents/components/TailorFlow', () => ({
  // Surface the injected seedGeneration id so we can assert DocumentsTab wires the
  // latest matching record (cold-entry hydration source). Also surface
  // `persistence.runId`/`runJobId` (F2 regression) — the self-describing-read
  // gate DocumentsTab applies to `applicationApply.applyRun`.
  TailorFlow: ({
    seedGeneration,
    onJobDescChange,
    persistence,
    resumeDocId,
  }: {
    seedGeneration?: { id: string };
    onJobDescChange?: (text: string) => void;
    persistence?: {
      runId: string | null;
      runJobId: string | null;
      wizardForm?: { resumeDocId?: string } | null;
    };
    resumeDocId?: string;
  }) => {
    state.capturedOnJobDescChange = onJobDescChange;
    return (
      <div
        data-testid={TEST_IDS.documents.tailorFlow}
        data-seedgenid={seedGeneration?.id ?? ''}
        data-runid={persistence?.runId ?? ''}
        data-runjobid={persistence?.runJobId ?? ''}
        data-resumedocid={resumeDocId ?? ''}
        // The PERSISTED form's id, distinct from the freshly-seeded prop above —
        // this is the one a deleted résumé can leave dangling.
        data-formdocid={persistence?.wizardForm?.resumeDocId ?? ''}
      />
    );
  },
}));

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: ({ gen }: { gen: { id: string } }) => (
    <div data-testid={TEST_IDS.documents.generationCard} data-genid={gen.id} />
  ),
}));

vi.mock('@/services', () => ({
  useApplication: () => mockUseApplication(),
  useSetApplicationStatus: () => ({ mutate: mockSetStatusMutate, isPending: false }),
  useUpdateApplication: () => ({ mutate: mockUpdateApplicationMutate, isPending: false }),
  useOpenExternal: () => ({ mutate: vi.fn() }),
  useRemoveApplication: () => ({ mutateAsync: mockRemoveMutateAsync, isPending: false }),
  useAcceptStatusEvent: () => ({ mutate: mockAcceptStatusEventMutate }),
  useRejectStatusEvent: () => ({ mutate: mockRejectStatusEventMutate }),
  useDocuments: () => ({ data: state.docsData, isLoading: false }),
  useDocumentText: () => ({ data: state.documentText, isLoading: false }),
  useImportJobUrl: () => ({
    mutate: mockImportJobUrlMutate,
    isPending: false,
    isError: state.importIsError,
  }),
  useResolveJobUrl: () => ({
    data: undefined,
    isFetching: state.resolveFetching,
    isError: false,
    isFetched: false,
    refetch: vi.fn(),
  }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => mockUseAiGenerations(),
}));

/** Render the page for `makeApp(overrides)` with no generations. */
export function renderLoaded(overrides: Partial<Application> = {}) {
  const app = makeApp(overrides);
  setLoaded(app);
  render(<ApplicationDetailPage />);
  return app;
}

/** Open the Timeline tab with `events` (default: a plain `makeApp`). */
export function renderTimeline(events: StatusEvent[], appOverrides: Partial<Application> = {}) {
  state.tab = 'timeline';
  setLoaded(makeApp(appOverrides), events);
  return render(<ApplicationDetailPage />);
}
