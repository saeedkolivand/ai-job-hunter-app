/**
 * AutopilotPage — ?focus deep-link consumption + board provenance (Priority 4)
 *
 * Strategy:
 *  - Effect-level test: stub every heavy sub-component (AutopilotCard, CreationWizard,
 *    ApplyPage, EmptyState, useAutopilotRun) so the page renders cheaply.
 *  - `useSearch` backed by a mutable variable so we can set it per-test.
 *  - `useAutopilots` + `useInvalidateAutopilots` are controlled mocks.
 *  - Assert: with `?focus=<id>`, `setAutopilot({ focusedId: focus })` is called
 *    (observed via session-store) and navigate({to:'/autopilot',search:{},replace:true})
 *    is called to clear the param.
 *  - Board provenance: AutopilotCard stub captures `onApply` so tests invoke
 *    handleApply directly and assert the `board` forwarded to saveFromPosting.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import { useSessionStore } from '@/store/session-store';

import { AutopilotPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

let currentSearch: Record<string, string | undefined> = {};
const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/routes/autopilot.index', () => ({
  Route: { useSearch: () => currentSearch },
}));

// ── Services ──────────────────────────────────────────────────────────────────

const mockInvalidateAutopilots = vi.fn();
const mockMutateAsync = vi.fn();
let mockAutopilotList: Autopilot[] = [];

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal();
  return {
    ...(orig as object),
    useAutopilots: () => ({ data: mockAutopilotList, isLoading: false }),
    useInvalidateAutopilots: () => mockInvalidateAutopilots,
    useSaveFromPosting: () => ({ mutateAsync: mockMutateAsync }),
  };
});

// ── Heavy sub-component stubs ─────────────────────────────────────────────────

// Captures the onApply callback so board-provenance tests can invoke handleApply directly.
let capturedOnApply: ((job: AutopilotFoundJob) => void) | null = null;
// Captures the focusedJobUrl prop so the Back-navigation flow can assert it's forwarded.
let capturedFocusedJobUrl: string | null | undefined = null;

vi.mock('@/features/autopilot/components/AutopilotCard', () => ({
  AutopilotCard: ({
    onApply,
    focusedJobUrl,
  }: {
    onApply: (job: AutopilotFoundJob) => void;
    focusedJobUrl?: string | null;
  }) => {
    capturedOnApply = onApply;
    capturedFocusedJobUrl = focusedJobUrl;
    return <div data-testid={TEST_IDS.autopilot.card} />;
  },
}));

vi.mock('@/features/autopilot/components/CreationWizard', () => ({
  CreationWizard: () => <div data-testid={TEST_IDS.autopilot.creationWizard} />,
}));

vi.mock('@/features/autopilot/components/EmptyState', () => ({
  EmptyState: () => <div data-testid={TEST_IDS.autopilot.emptyState} />,
}));

vi.mock('@/features/autopilot/hooks/useAutopilotRun', () => ({
  useAutopilotRun: () => ({
    runStates: {},
    stepLogs: {},
    error: null,
    setError: vi.fn(),
    handleRun: vi.fn(),
    handleTogglePause: vi.fn(),
    handleDelete: vi.fn(),
  }),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: React.ReactNode }) => (
    <div data-testid={TEST_IDS.layout.pageTransition}>{children}</div>
  ),
}));

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useSessionStore.setState((s) => ({
    autopilot: {
      ...s.autopilot,
      focusedId: null,
      focusedJobUrl: null,
      lastAppliedId: null,
      lastAppliedJobUrl: null,
      creating: false,
    },
  }));
  mockNavigate.mockReset();
  mockInvalidateAutopilots.mockReset();
  mockMutateAsync.mockReset();
  mockAutopilotList = [];
  capturedOnApply = null;
  capturedFocusedJobUrl = null;
  currentSearch = {};
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('AutopilotPage — ?focus consumption', () => {
  it('sets autopilot.focusedId in session store when ?focus is present', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedId).toBe('ap-42');
  });

  it('clears the ?focus URL param via navigate({to:/autopilot,search:{},replace:true})', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ to: '/autopilot', search: {}, replace: true })
    );
  });

  it('calls useInvalidateAutopilots() to refresh the list when ?focus is set', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockInvalidateAutopilots).toHaveBeenCalledTimes(1);
  });

  it('does nothing when ?focus is absent', async () => {
    currentSearch = {};

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockInvalidateAutopilots).not.toHaveBeenCalled();
    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedId).toBeNull();
  });
});

describe('AutopilotPage — lastAppliedId (re-expand on Back)', () => {
  it('promotes lastAppliedId to focusedId on mount and clears it', async () => {
    // Simulates returning from an Apply deep-link: the page should re-focus the
    // autopilot the user applied from so its found-jobs list re-expands.
    useSessionStore.setState((s) => ({ autopilot: { ...s.autopilot, lastAppliedId: 'ap-7' } }));

    await act(async () => {
      render(<AutopilotPage />);
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedId).toBe('ap-7');
    expect(autopilot.lastAppliedId).toBeNull();
  });

  it('promotes lastAppliedJobUrl to focusedJobUrl on mount and clears it', async () => {
    // Same Back-navigation flow, but also carrying the specific job url so the
    // card can scroll to that row instead of just the header.
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        lastAppliedId: 'ap-7',
        lastAppliedJobUrl: 'https://example.com/job/1',
      },
    }));

    await act(async () => {
      render(<AutopilotPage />);
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedJobUrl).toBe('https://example.com/job/1');
    expect(autopilot.lastAppliedJobUrl).toBeNull();
  });
});

// Minimal fixtures for board-provenance tests
const makeAutopilot = (boards: string[]): Autopilot => ({
  _id: 'ap-1',
  name: 'Test autopilot',
  status: 'active',
  target: { boards, query: 'engineer', pages: 1 },
  filter: { minMatchScore: 0 },
  schedule: 'manual',
  totalFound: 0,
  totalApplied: 0,
  createdAt: 0,
  updatedAt: 0,
});

const makeFoundJob = (board?: string): AutopilotFoundJob => ({
  title: 'Engineer',
  company: 'Acme',
  url: 'https://example.com/job/1',
  foundAt: 0,
  board,
});

describe('AutopilotPage — handleApply board provenance', () => {
  it('uses job.board when present, ignoring ap.target.boards[0]', async () => {
    mockAutopilotList = [makeAutopilot(['indeed'])];
    mockMutateAsync.mockResolvedValue({ id: 'app-99' });

    await act(async () => {
      render(<AutopilotPage />);
    });

    if (!capturedOnApply) throw new Error('AutopilotCard onApply was not captured');
    await act(async () => {
      (capturedOnApply as (job: AutopilotFoundJob) => void)(makeFoundJob('linkedin'));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith(expect.objectContaining({ board: 'linkedin' }));
  });

  it('falls back to ap.target.boards[0] when job.board is absent', async () => {
    mockAutopilotList = [makeAutopilot(['indeed'])];
    mockMutateAsync.mockResolvedValue({ id: 'app-99' });

    await act(async () => {
      render(<AutopilotPage />);
    });

    if (!capturedOnApply) throw new Error('AutopilotCard onApply was not captured');
    await act(async () => {
      (capturedOnApply as (job: AutopilotFoundJob) => void)(makeFoundJob(undefined));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith(expect.objectContaining({ board: 'indeed' }));
  });

  it('falls back to AGGREGATOR_BOARD_ID when both job.board and boards[0] are absent', async () => {
    mockAutopilotList = [makeAutopilot([])];
    mockMutateAsync.mockResolvedValue({ id: 'app-99' });

    await act(async () => {
      render(<AutopilotPage />);
    });

    if (!capturedOnApply) throw new Error('AutopilotCard onApply was not captured');
    await act(async () => {
      (capturedOnApply as (job: AutopilotFoundJob) => void)(makeFoundJob(undefined));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith(expect.objectContaining({ board: 'aggregator' }));
  });
});

describe('AutopilotPage — focusedJobUrl plumbing', () => {
  it('forwards focusedJobUrl to the focused AutopilotCard', async () => {
    mockAutopilotList = [makeAutopilot(['indeed'])]; // _id: 'ap-1'
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        lastAppliedId: 'ap-1',
        lastAppliedJobUrl: 'https://example.com/job/1',
      },
    }));

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(capturedFocusedJobUrl).toBe('https://example.com/job/1');
  });

  it('sets lastAppliedJobUrl alongside lastAppliedId on Apply', async () => {
    mockAutopilotList = [makeAutopilot(['indeed'])];
    mockMutateAsync.mockResolvedValue({ id: 'app-99' });

    await act(async () => {
      render(<AutopilotPage />);
    });

    if (!capturedOnApply) throw new Error('AutopilotCard onApply was not captured');
    await act(async () => {
      (capturedOnApply as (job: AutopilotFoundJob) => void)(makeFoundJob('linkedin'));
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.lastAppliedId).toBe('ap-1');
    expect(autopilot.lastAppliedJobUrl).toBe('https://example.com/job/1');
  });
});
