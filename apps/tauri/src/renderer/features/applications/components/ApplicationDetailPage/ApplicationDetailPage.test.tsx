/**
 * ApplicationDetailPage — generation matching + not-found + loading tests
 *
 * Strategy:
 *  - All service hooks (`useApplication`, `useSetApplicationStatus`,
 *    `useUpdateApplication`, `useOpenExternal`) are mocked at module level
 *    via the `@/services` barrel so no IPC / QueryClient / AppClientProvider
 *    tree is needed.
 *  - `useAiGenerations` is mocked separately (not in the barrel).
 *  - `Route.useParams` is mocked to return `{ id: 'app-1' }` — renders
 *    without a RouterProvider.
 *  - `useNavigate` is mocked to a no-op fn.
 *  - `useSessionStore` selector form is mocked by unwrapping the selector:
 *    `(sel) => sel({ setAIGenerate: vi.fn() })`.
 *  - `GenerationCard` is stubbed to a deterministic marker so assertions are
 *    cheap and don't pull in that component's deps.
 *  - `PageShell` is stubbed to render children inside a wrapper div.
 *  - `useFormatRelativeTime` is mocked to `() => () => ''` to keep it inert.
 *  - `@ajh/translations` returns keys as-is.
 *  - noUncheckedIndexedAccess: all array accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';
import type { AiGenerationRecord } from '@ajh/shared/ipc';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router — render standalone (no RouterProvider) ────────────────────────────

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@/routes/applications.$id', () => ({
  Route: { useParams: () => ({ id: 'app-1' }) },
}));

// ── Session store — unwrap selector form ──────────────────────────────────────

vi.mock('@/store/session-store', () => ({
  useSessionStore: <T,>(sel: (s: { setAIGenerate: ReturnType<typeof vi.fn> }) => T): T =>
    sel({ setAIGenerate: vi.fn() }),
}));

// ── Hooks ─────────────────────────────────────────────────────────────────────

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => () => '',
}));

// ── Layout stub ───────────────────────────────────────────────────────────────

vi.mock('@/components/layout/PageShell', () => ({
  PageShell: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="page-shell">{children}</div>
  ),
}));

// ── GenerationCard stub ───────────────────────────────────────────────────────

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: ({ gen }: { gen: AiGenerationRecord }) => (
    <div data-testid="generation-card" data-genid={gen.id} />
  ),
}));

// ── Service hooks — controlled mocks ─────────────────────────────────────────

const mockUseApplication = vi.fn();
const mockUseAiGenerations = vi.fn();
const mockUpdateApplicationMutate = vi.fn();

vi.mock('@/services', () => ({
  useApplication: () => mockUseApplication(),
  useSetApplicationStatus: () => ({
    mutateAsync: vi.fn().mockResolvedValue(undefined),
    isPending: false,
  }),
  useUpdateApplication: () => ({
    mutate: mockUpdateApplicationMutate,
    isPending: false,
  }),
  useOpenExternal: () => ({
    mutate: vi.fn(),
  }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => mockUseAiGenerations(),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeApp(overrides: Partial<Application> = {}): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

/**
 * Minimal generation fixture. Only `id` and `jobUrl` are read by the
 * component's filter; `GenerationCard` itself is stubbed, so the rest
 * of the AiGenerationRecord fields are cast placeholders.
 */
function makeGen(overrides: { id: string; jobUrl: string }): AiGenerationRecord {
  return {
    id: overrides.id,
    jobUrl: overrides.jobUrl,
    createdAt: 0,
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    targetLanguage: 'en',
    mismatch: false,
    topRequirements: [],
    mode: 'standard',
    resumeText: '',
    coverLetterText: '',
    jobAd: '',
    board: '',
    applicationAnswers: [],
    companyBrief: '',
  };
}

// ── Import component under test (after all mocks) ─────────────────────────────

import { ApplicationDetailPage } from './index';

// ── Reset between tests ───────────────────────────────────────────────────────

beforeEach(() => {
  mockUseApplication.mockReset();
  mockUseAiGenerations.mockReset();
  mockUpdateApplicationMutate.mockClear();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — generation matching', () => {
  it('renders a GenerationCard for a generation whose jobUrl matches the application', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [
        makeGen({ id: 'gen-1', jobUrl: 'https://acme.com/job/1' }),
        makeGen({ id: 'gen-2', jobUrl: 'https://other.com/x' }),
      ],
    });

    render(<ApplicationDetailPage />);

    const cards = screen.getAllByTestId('generation-card');
    expect(cards).toHaveLength(1);

    const card = cards[0];
    expect(card).toBeDefined();
    expect(card?.getAttribute('data-genid')).toBe('gen-1');

    // The "Generate documents" CTA must NOT be shown when a match exists.
    expect(screen.queryByText('applications.detail.generateDocs')).not.toBeInTheDocument();
  });

  it('shows the Generate documents CTA when no generation matches', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [makeGen({ id: 'gen-x', jobUrl: 'https://different.com/job/99' })],
    });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.generateDocs')).toBeInTheDocument();
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
  });

  it('shows the Generate documents CTA when the generations list is empty', () => {
    const app = makeApp({ jobUrl: 'https://acme.com/job/1' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.generateDocs')).toBeInTheDocument();
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
  });

  it('does NOT match generations when the application jobUrl is empty', () => {
    // The component's guard: `appUrl !== '' && g.jobUrl.trim() === appUrl`
    const app = makeApp({ jobUrl: '' });

    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({
      data: [makeGen({ id: 'gen-z', jobUrl: '' })],
    });

    render(<ApplicationDetailPage />);

    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
    expect(screen.getByText('applications.detail.generateDocs')).toBeInTheDocument();
  });
});

describe('ApplicationDetailPage — save-on-blur (editable fields)', () => {
  function renderLoaded(app: Application) {
    mockUseApplication.mockReturnValue({
      data: { application: app, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });
    render(<ApplicationDetailPage />);
  }

  it('blurring the notes field with an UNCHANGED value does NOT call the update mutation', () => {
    // The notes field is seeded from `application.notes`. Blurring without
    // editing leaves the buffer equal to the seed → no mutation.
    const app = makeApp({ notes: 'existing note' });
    renderLoaded(app);

    // t() returns keys, so the textarea's accessible name is the label key.
    const notes = screen.getByLabelText('applications.detail.notesLabel');
    fireEvent.blur(notes);

    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();
  });

  it('blurring the notes field with a CHANGED value DOES call the update mutation', () => {
    const app = makeApp({ id: 'app-edit-1', notes: 'existing note' });
    renderLoaded(app);

    const notes = screen.getByLabelText('applications.detail.notesLabel');
    fireEvent.change(notes, { target: { value: 'new note text' } });
    fireEvent.blur(notes);

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-edit-1',
      notes: 'new note text',
    });
  });
});

describe('ApplicationDetailPage — not-found / error state', () => {
  it('shows the not-found error state when application is null', () => {
    mockUseApplication.mockReturnValue({
      data: { application: null, events: [] },
      isLoading: false,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.notFound')).toBeInTheDocument();
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
  });

  it('shows the not-found error state when isError=true', () => {
    mockUseApplication.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<ApplicationDetailPage />);

    expect(screen.getByText('applications.detail.notFound')).toBeInTheDocument();
  });
});

describe('ApplicationDetailPage — loading state', () => {
  it('renders skeletons while loading and suppresses content', () => {
    mockUseApplication.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
    });
    mockUseAiGenerations.mockReturnValue({ data: [] });

    const { container } = render(<ApplicationDetailPage />);

    // No GenerationCard and no not-found text while loading.
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
    expect(screen.queryByText('applications.detail.notFound')).not.toBeInTheDocument();

    // RowSkeleton / CardSkeleton render real elements with animate-skeleton class.
    const skeletonShimmer = container.querySelectorAll('.animate-skeleton');
    expect(skeletonShimmer.length).toBeGreaterThan(0);
  });
});
