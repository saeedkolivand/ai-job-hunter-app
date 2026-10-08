/** AutopilotPage — first run after creation: manual schedules never auto-run (#1397). */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import type { Autopilot } from '@ajh/shared';

import { useSessionStore } from '@/store/session-store';

import { AutopilotPage } from './index';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@/routes/autopilot.index', () => ({ Route: { useSearch: () => ({}) } }));
vi.mock('@/services', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  useAutopilots: () => ({ data: [], isLoading: false }),
  useInvalidateAutopilots: () => vi.fn(),
  useSaveFromPosting: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/features/autopilot/components/AutopilotCard', () => ({ AutopilotCard: () => null }));
vi.mock('@/features/autopilot/components/EmptyState', () => ({ EmptyState: () => null }));
vi.mock('@/components/job/BestMatchesPreview', () => ({ BestMatchesPreview: () => null }));
vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

let onDone: ((ap: Autopilot) => void) | null = null;
vi.mock('@/features/autopilot/components/CreationWizard', () => ({
  CreationWizard: (p: { onDone: (ap: Autopilot) => void }) => {
    onDone = p.onDone;
    return null;
  },
}));

const mockHandleRun = vi.fn();
vi.mock('@/features/autopilot/hooks/useAutopilotRun', () => ({
  useAutopilotRun: () => ({
    runStates: {},
    stepLogs: {},
    error: null,
    setError: vi.fn(),
    handleRun: mockHandleRun,
    handleTogglePause: vi.fn(),
    handleDelete: vi.fn(),
  }),
}));

beforeEach(() => {
  mockHandleRun.mockReset();
  onDone = null;
});

async function create(schedule: Autopilot['schedule']) {
  useSessionStore.setState((s) => ({
    autopilot: { ...s.autopilot, creating: true, editingId: null },
  }));
  await act(async () => {
    render(<AutopilotPage />);
  });
  act(() => onDone?.({ _id: 'ap-new', schedule } as unknown as Autopilot));
}

describe('AutopilotPage — first run after creation', () => {
  it('does NOT auto-run a manual autopilot', async () => {
    await create('manual');
    expect(mockHandleRun).not.toHaveBeenCalled();
  });

  it('keeps the immediate first run for scheduled autopilots (#44)', async () => {
    await create('daily');
    expect(mockHandleRun).toHaveBeenCalledWith('ap-new');
  });
});
