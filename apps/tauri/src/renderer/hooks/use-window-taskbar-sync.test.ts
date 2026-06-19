/**
 * useWindowTaskbarSync — unit tests
 *
 * Strategy:
 *  - useWindowControls is mocked to provide fully-controlled spies.
 *  - useJobQueue (via @/services/use-jobs/use-jobs) is mocked to return a
 *    controlled job list; useJobEvents callback is captured from the mock so
 *    tests can fire synthetic completion events synchronously.
 *  - renderHook from @testing-library/react drives the hook lifecycle.
 *  - vi.hoisted() is used for all spies referenced inside vi.mock() factories.
 *
 * Coverage:
 *  1. Running job with determinate progress → setTaskbarProgress(p).
 *  2. Running job with zero/no progress (AI streaming) → setTaskbarProgress(-1).
 *  3. Idle (no running jobs) → setTaskbarProgress(null).
 *  4. Redundant-call guard: same computed progress → no extra call for that value;
 *     no spurious null dispatch between rerenders (flicker regression guard).
 *  5. Completion while unfocused → flashAttention called.
 *  6. Completion while focused → flashAttention NOT called.
 *  7. macOS + unfocused completion → showApp awaited BEFORE flashAttention.
 *  8. Non-macOS completion → showApp NOT called.
 *  9. Unmount while job running → setTaskbarProgress(null) called for cleanup.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { JobRecord } from '@ajh/shared';

// ── hoisted spies — must be declared via vi.hoisted() before vi.mock hoisting ─

const { mockSetTaskbarProgress, mockFlashAttention, mockShowApp, mockIsFocused } = vi.hoisted(
  () => ({
    mockSetTaskbarProgress: vi.fn().mockResolvedValue(undefined),
    mockFlashAttention: vi.fn().mockResolvedValue(undefined),
    mockShowApp: vi.fn().mockResolvedValue(undefined),
    mockIsFocused: vi.fn().mockResolvedValue(true),
  })
);

// isMacos is a plain boolean read at call-time inside the factory closure.
// It is NOT referenced in the factory directly — the factory wraps it in a
// getter via a captured mutable container so it can be changed per-test.
const isMacosContainer = { value: false };

// A single stable controls object — mirrors useMemo([])'s referential stability
// so the unmount-only effect (dep: controls) doesn't fire between rerenders.
const stableControls = {
  get isMacos() {
    return isMacosContainer.value;
  },
  isFocused: mockIsFocused,
  setTaskbarProgress: mockSetTaskbarProgress,
  flashAttention: mockFlashAttention,
  showApp: mockShowApp,
  toggleMaximize: vi.fn(),
  foreground: vi.fn(),
};

vi.mock('@/services/use-window-controls/use-window-controls', () => ({
  useWindowControls: () => stableControls,
}));

// ── Controlled mock for useJobQueue and useJobEvents ─────────────────────────

type JobEventCallback = (event: { type: string }) => void;

let mockJobs: Partial<JobRecord>[] = [];
let capturedJobEventCallback: JobEventCallback | undefined;

vi.mock('@/services/use-jobs/use-jobs', () => ({
  useJobQueue: () => ({ data: mockJobs }),
  useJobEvents: (cb?: JobEventCallback) => {
    // Capture the callback so tests can fire synthetic events.
    capturedJobEventCallback = cb;
  },
}));

// ── import after mocks ─────────────────────────────────────────────────────────

import { useWindowTaskbarSync } from './use-window-taskbar-sync';

// ── helpers ───────────────────────────────────────────────────────────────────

function makeJob(overrides: Partial<JobRecord> = {}): Partial<JobRecord> {
  return {
    id: 'j1',
    kind: 'scrape.board',
    status: 'running',
    progress: 0,
    ...overrides,
  };
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockSetTaskbarProgress.mockClear();
  mockFlashAttention.mockClear();
  mockShowApp.mockClear();
  mockIsFocused.mockResolvedValue(true);
  isMacosContainer.value = false;
  mockJobs = [];
  capturedJobEventCallback = undefined;
});

// ── Progress sync ─────────────────────────────────────────────────────────────

describe('useWindowTaskbarSync — progress sync', () => {
  it('running job with determinate progress → setTaskbarProgress(p)', () => {
    mockJobs = [makeJob({ status: 'running', progress: 0.4 })];
    renderHook(() => useWindowTaskbarSync());
    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(0.4);
  });

  it('running job with zero progress (AI streaming) → setTaskbarProgress(-1)', () => {
    mockJobs = [makeJob({ status: 'streaming', progress: 0 })];
    renderHook(() => useWindowTaskbarSync());
    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(-1);
  });

  it('running job with undefined progress → setTaskbarProgress(-1)', () => {
    mockJobs = [makeJob({ status: 'running', progress: undefined })];
    renderHook(() => useWindowTaskbarSync());
    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(-1);
  });

  it('idle (no running jobs) → setTaskbarProgress(null)', () => {
    mockJobs = [];
    renderHook(() => useWindowTaskbarSync());
    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(null);
  });

  it('queued-only jobs are treated as idle → setTaskbarProgress(null)', () => {
    mockJobs = [makeJob({ status: 'queued', progress: 0 })];
    renderHook(() => useWindowTaskbarSync());
    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(null);
  });
});

// ── Redundant-call guard ───────────────────────────────────────────────────────
//
// The guard (lastProgressRef) prevents duplicate setTaskbarProgress calls for
// the same computed value within one effect activation. We verify that the
// specific progress value that was first dispatched is never sent a second time
// when jobs change but the computed progress stays the same.

describe('useWindowTaskbarSync — redundant-call guard', () => {
  it('guard: when computed progress stays the same after a jobs change, the value is not re-dispatched', () => {
    mockJobs = [makeJob({ id: 'j1', status: 'running', progress: 0.4 })];
    const { rerender } = renderHook(() => useWindowTaskbarSync());

    const callsWith04Before = mockSetTaskbarProgress.mock.calls.filter(
      (args) => args[0] === 0.4
    ).length;
    expect(callsWith04Before).toBe(1);

    // Different job record, same progress value — guard should prevent re-dispatch.
    mockJobs = [makeJob({ id: 'j2', status: 'running', progress: 0.4 })];
    rerender();

    const callsWith04After = mockSetTaskbarProgress.mock.calls.filter(
      (args) => args[0] === 0.4
    ).length;
    // Still only one call with 0.4 — guard fired.
    expect(callsWith04After).toBe(1);
  });

  it('no spurious null dispatch between rerenders (flicker regression)', () => {
    // Pre-fix: the cleanup inside the progress effect ran on every jobs change,
    // emitting setTaskbarProgress(null) before each new value → flicker.
    // Post-fix: null is only dispatched on unmount, never mid-session.
    mockJobs = [makeJob({ id: 'j1', status: 'running', progress: 0.4 })];
    const { rerender } = renderHook(() => useWindowTaskbarSync());

    // Progress changes — the old cleanup would have fired null here.
    mockJobs = [makeJob({ id: 'j1', status: 'running', progress: 0.8 })];
    rerender();

    const nullCalls = mockSetTaskbarProgress.mock.calls.filter((args) => args[0] === null);
    expect(nullCalls).toHaveLength(0);
  });

  it('guard: when computed progress changes, the new value IS dispatched', () => {
    mockJobs = [makeJob({ status: 'running', progress: 0.4 })];
    const { rerender } = renderHook(() => useWindowTaskbarSync());

    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(0.4);

    mockJobs = [makeJob({ status: 'running', progress: 0.8 })];
    rerender();

    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(0.8);
  });
});

// ── Unmount cleanup ───────────────────────────────────────────────────────────

describe('useWindowTaskbarSync — unmount cleanup', () => {
  it('unmount while a job is running → setTaskbarProgress(null)', () => {
    mockJobs = [makeJob({ status: 'running', progress: 0.5 })];
    const { unmount } = renderHook(() => useWindowTaskbarSync());
    mockSetTaskbarProgress.mockClear();

    unmount();

    expect(mockSetTaskbarProgress).toHaveBeenCalledWith(null);
  });
});

// ── Attention on completion ───────────────────────────────────────────────────

describe('useWindowTaskbarSync — attention on job terminal events', () => {
  it('job.completed while unfocused → flashAttention called', async () => {
    mockIsFocused.mockResolvedValue(false);
    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.completed' });
    });

    await vi.waitFor(() => {
      expect(mockFlashAttention).toHaveBeenCalledOnce();
    });
  });

  it('job.failed while unfocused → flashAttention called', async () => {
    mockIsFocused.mockResolvedValue(false);
    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.failed' });
    });

    await vi.waitFor(() => {
      expect(mockFlashAttention).toHaveBeenCalledOnce();
    });
  });

  it('job.completed while focused → flashAttention NOT called', async () => {
    mockIsFocused.mockResolvedValue(true);
    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.completed' });
    });

    // Wait until the focus check has actually resolved before asserting
    // suppression — proves focus=true caused the skip, not a timing race.
    await vi.waitFor(() => expect(mockIsFocused).toHaveBeenCalled());
    expect(mockFlashAttention).not.toHaveBeenCalled();
  });

  it('non-terminal event (job.progress) is ignored — flashAttention not called', async () => {
    mockIsFocused.mockResolvedValue(false);
    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.progress' });
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(mockFlashAttention).not.toHaveBeenCalled();
  });
});

// ── macOS ordering: showApp before flashAttention ─────────────────────────────

describe('useWindowTaskbarSync — macOS call ordering', () => {
  it('macOS + unfocused → showApp called BEFORE flashAttention', async () => {
    isMacosContainer.value = true;
    mockIsFocused.mockResolvedValue(false);

    const callOrder: string[] = [];
    mockShowApp.mockImplementation(async () => {
      callOrder.push('showApp');
    });
    mockFlashAttention.mockImplementation(async () => {
      callOrder.push('flashAttention');
    });

    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.completed' });
    });

    await vi.waitFor(() => {
      expect(callOrder).toEqual(['showApp', 'flashAttention']);
    });
  });

  it('non-macOS + unfocused → showApp NOT called, flashAttention IS called', async () => {
    isMacosContainer.value = false;
    mockIsFocused.mockResolvedValue(false);

    renderHook(() => useWindowTaskbarSync());

    await act(async () => {
      capturedJobEventCallback?.({ type: 'job.completed' });
    });

    await vi.waitFor(() => {
      expect(mockFlashAttention).toHaveBeenCalledOnce();
    });
    expect(mockShowApp).not.toHaveBeenCalled();
  });
});
