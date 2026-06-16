/**
 * DocumentsPage — selection-mode tests
 *
 * Strategy (mirrors ApplicationsPage.test.tsx):
 *  - The three service hooks (`useAiGenerations`, `useInteractions`,
 *    `useRemoveAiGenerationsBulk`) are mocked at the module level so no IPC /
 *    QueryClient / AppClientProvider tree is needed.
 *  - `useSessionStore` (Zustand) is used directly: its initial `resumes` slice
 *    is `{ tab: 'resumes', filter: '' }`, which is the Résumés-tab state we want.
 *  - `motion/react` is replaced with plain fragments so animation code never runs.
 *  - `@ajh/translations` returns keys as-is, so the "Select" / "Done" buttons and
 *    the Select-all checkbox are matched by their translation keys.
 *  - `GenerationCard` is stubbed to a deterministic marker that records whether
 *    it received an `onToggleSelect` handler (`data-selectable`) — this is how we
 *    assert each card's checkbox is hidden outside selection mode.
 *  - `InteractionRow` is stubbed (the Résumés tab never renders it, but the
 *    import must resolve without pulling its deps).
 *
 * Behaviour under test (the selection-mode toggle):
 *  - default (selectionMode=false): no Select-all checkbox, GenerationCards are
 *    NOT selectable, only a "Select" button shows.
 *  - clicking "Select": reveals the Select-all checkbox + a "Done" button, and
 *    GenerationCards become selectable (receive onToggleSelect).
 *  - clicking "Done": returns to the default state.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';

import { useSessionStore } from '@/store/session-store';

import { DocumentsPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react ──────────────────────────────────────────────────────────────

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── Service hooks — controlled mocks ──────────────────────────────────────────

const mockUseAiGenerations = vi.fn();

vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => mockUseAiGenerations(),
  useRemoveAiGenerationsBulk: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/services/use-postings', () => ({
  useInteractions: () => ({ data: [], isLoading: false, refetch: vi.fn() }),
}));

// ── GenerationCard stub — records whether it is selectable ────────────────────

vi.mock('@/features/documents/components/GenerationCard', () => ({
  GenerationCard: ({
    gen,
    selected,
    onToggleSelect,
  }: {
    gen: AiGenerationRecord;
    selected: boolean;
    onToggleSelect?: (id: string) => void;
  }) => (
    <div
      data-testid="generation-card"
      data-genid={gen.id}
      data-selected={selected ? '1' : '0'}
      // The whole point of the change: the card is only "selectable" (renders its
      // checkbox) when DocumentsPage passes onToggleSelect, i.e. in selection mode.
      data-selectable={onToggleSelect ? '1' : '0'}
    >
      {gen.jobTitle}
    </div>
  ),
}));

// ── InteractionRow stub (Résumés tab never renders it; import must resolve) ────

vi.mock('@/features/documents/components/InteractionRow', () => ({
  InteractionRow: () => <div data-testid="interaction-row" />,
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeGen(overrides: Partial<AiGenerationRecord>): AiGenerationRecord {
  return {
    id: 'gen-1',
    createdAt: 1000,
    candidateName: 'Jane',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    targetLanguage: 'en',
    mismatch: false,
    topRequirements: [],
    mode: 'tailor',
    resumeText: 'a resume',
    coverLetterText: '',
    jobAd: '',
    jobUrl: '',
    board: 'linkedin',
    applicationAnswers: [],
    companyBrief: '',
    interviewQuestions: [],
    ...overrides,
  };
}

const GENS: AiGenerationRecord[] = [
  makeGen({ id: 'g1', jobTitle: 'Role One', resumeText: 'r1' }),
  makeGen({ id: 'g2', jobTitle: 'Role Two', resumeText: 'r2' }),
];

const SELECT_ALL = 'resumes.select.selectAll';
const SELECT_START = 'resumes.select.start';
const SELECT_DONE = 'resumes.select.done';

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  // Reset to the Résumés tab with no filter (the slice default).
  useSessionStore.setState((s) => ({
    resumes: { ...s.resumes, tab: 'resumes', filter: '' },
  }));
  mockUseAiGenerations.mockReset();
  mockUseAiGenerations.mockReturnValue({ data: GENS });
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('DocumentsPage — selection mode', () => {
  it('hides the Select-all checkbox and per-card checkboxes by default', () => {
    render(<DocumentsPage />);

    // No Select-all checkbox in the header before entering selection mode.
    expect(screen.queryByLabelText(SELECT_ALL)).not.toBeInTheDocument();

    // The "Select" entry button is shown; "Done" is not.
    expect(screen.getByRole('button', { name: SELECT_START })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: SELECT_DONE })).not.toBeInTheDocument();

    // Cards render but are NOT selectable (no onToggleSelect → no checkbox).
    const cards = screen.getAllByTestId('generation-card');
    expect(cards).toHaveLength(2);
    cards.forEach((c) => expect(c).toHaveAttribute('data-selectable', '0'));
  });

  it('reveals the Select-all checkbox + Done button and makes cards selectable after clicking "Select"', () => {
    render(<DocumentsPage />);

    fireEvent.click(screen.getByRole('button', { name: SELECT_START }));

    // Select-all checkbox + Done button now present; the "Select" entry button is gone.
    expect(screen.getByLabelText(SELECT_ALL)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: SELECT_DONE })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: SELECT_START })).not.toBeInTheDocument();

    // Every card is now selectable (received onToggleSelect).
    screen
      .getAllByTestId('generation-card')
      .forEach((c) => expect(c).toHaveAttribute('data-selectable', '1'));
  });

  it('exits selection mode (hides checkbox + cards no longer selectable) after clicking "Done"', () => {
    render(<DocumentsPage />);

    fireEvent.click(screen.getByRole('button', { name: SELECT_START }));
    expect(screen.getByLabelText(SELECT_ALL)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: SELECT_DONE }));

    // Back to the default state.
    expect(screen.queryByLabelText(SELECT_ALL)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: SELECT_START })).toBeInTheDocument();
    screen
      .getAllByTestId('generation-card')
      .forEach((c) => expect(c).toHaveAttribute('data-selectable', '0'));
  });

  it('does not show the Select button when there are no documents', () => {
    mockUseAiGenerations.mockReturnValue({ data: [] });

    render(<DocumentsPage />);

    expect(screen.queryByRole('button', { name: SELECT_START })).not.toBeInTheDocument();
    expect(screen.queryByLabelText(SELECT_ALL)).not.toBeInTheDocument();
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
  });

  it('keeps the Select button when a search filter matches no documents', () => {
    // Tab has documents (GENS), but the active filter matches none of them.
    useSessionStore.setState((s) => ({
      resumes: { ...s.resumes, tab: 'resumes', filter: 'zzz-no-match' },
    }));

    render(<DocumentsPage />);

    // The header Select control stays visible because the tab has documents,
    // even though the filtered list is empty.
    expect(screen.getByRole('button', { name: SELECT_START })).toBeInTheDocument();

    // No cards survive the filter, and the no-results empty state is shown.
    expect(screen.queryByTestId('generation-card')).not.toBeInTheDocument();
    expect(screen.getByText('resumes.noResults')).toBeInTheDocument();
  });

  it('checks every card via Select-all once in selection mode', () => {
    render(<DocumentsPage />);

    fireEvent.click(screen.getByRole('button', { name: SELECT_START }));

    // Before Select-all: no card is selected.
    screen
      .getAllByTestId('generation-card')
      .forEach((c) => expect(c).toHaveAttribute('data-selected', '0'));

    fireEvent.click(screen.getByLabelText(SELECT_ALL));

    // After Select-all: every card is selected.
    screen
      .getAllByTestId('generation-card')
      .forEach((c) => expect(c).toHaveAttribute('data-selected', '1'));
  });
});
