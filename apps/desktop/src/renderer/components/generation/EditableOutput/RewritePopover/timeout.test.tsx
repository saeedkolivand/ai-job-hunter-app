/**
 * RewritePopover — timeout and retry window.
 *
 * Verifies the client-side safety net added in Fix #8b:
 *  - A stalled provider stream is aborted at the EFFORT-SCALED bound the shared
 *    stream helper uses (never a hardcoded 60 s, which sat below the backend's
 *    own deadline for the same request) and the error state is surfaced.
 *  - The timeout is cleared when the stream resolves normally — no spurious
 *    error fires after a successful rewrite.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen } from '@testing-library/react';

import { rewriteSelection } from '@/lib/generate';

import { RESOLVED_TIMEOUT_MS } from './test-mocks';
import { acceptButton, mockStall, renderPopover, runInstruction } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('motion/react', async () => (await import('./test-mocks')).motionMock());
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-mocks')).uiMock(await importOriginal())
);

describe('RewritePopover — timeout', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockStall();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('does NOT abort at the deleted 60 s constant, and surfaces aiGenerate.rewrite.failed at the resolved bound', async () => {
    renderPopover();

    // Trigger run() via the first preset chip.
    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));

    // 60 s used to kill the stream here while the backend was still streaming
    // (its own deadline is 300 s scaled by effort). Nothing may happen yet.
    await act(async () => {
      vi.advanceTimersByTime(60_001);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();

    // Past the bound `resolveRewriteTimeoutMs` actually returned: abort + error.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS);
      // Flush the abort-event → rejection → .catch microtask chain.
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByText('aiGenerate.rewrite.failed')).toBeTruthy();
  });

  it('shows the still-working line once a stream passes ~20 s, and drops it when the stream lands', async () => {
    renderPopover();

    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));
    expect(screen.queryByText('aiGenerate.rewrite.stillWorking')).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(20_001);
      await Promise.resolve();
    });
    expect(screen.getByText('aiGenerate.rewrite.stillWorking')).toBeTruthy();

    // The stall mock rejects on abort; abort via the resolved bound and the
    // line must go away with the stream.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByText('aiGenerate.rewrite.stillWorking')).toBeNull();
  });

  it('does NOT show error and clears streaming when the stream resolves before timeout', async () => {
    // Override for this test: resolves immediately instead of stalling.
    vi.mocked(rewriteSelection).mockResolvedValueOnce('rewritten text');

    renderPopover();

    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));

    // Let the resolved promise flush through .then / .finally.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // Advance past what would have been the timeout — must NOT fire the error
    // since clearTimeout(timeoutId) ran in .finally.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS + 1);
      await Promise.resolve();
    });

    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    // The rewrite result is displayed.
    expect(screen.getByText('rewritten text')).toBeTruthy();
  });
});

describe('RewritePopover — retry gets its own timeout window (HIGH)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('re-arms the timeout before the one re-ask, so a retry that itself takes close to the full window still settles instead of hitting the shared deadline', async () => {
    const over = 'x'.repeat(30);
    const insideLimit = 'y'.repeat(10);
    let resolveFirst!: (value: string) => void;
    let resolveRetry!: (value: string) => void;
    let calls = 0;
    vi.mocked(rewriteSelection).mockImplementation(
      ({ signal }: { signal?: AbortSignal }) =>
        new Promise<string>((resolve, reject) => {
          calls += 1;
          const thisCall = calls;
          signal?.addEventListener('abort', () =>
            reject(new DOMException('Aborted', 'AbortError'))
          );
          if (thisCall === 1) resolveFirst = resolve;
          else resolveRetry = resolve;
        })
    );

    renderPopover();
    await runInstruction('rewrite this under 20 characters');

    // Burn almost the entire window on the FIRST attempt before it resolves —
    // mirrors the design's own measured numbers (a single long stream up to
    // ~152 s against the ~300 s resolved ceiling here).
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS - 1_000);
    });

    await act(async () => {
      resolveFirst(over);
      // Flush the `.then(async (first) => …)` re-ask leg: the exceeds-limit
      // check, the timer re-arm, and the retry's own `attempt()` call.
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(2);

    // The retry now runs almost the FULL window on its own clock. With the bug
    // (a shared, un-reset timer) the ORIGINAL deadline — only ~1 s away at this
    // point — would fire here and abort the retry, discarding `over`.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS - 1_000);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();

    await act(async () => {
      resolveRetry(insideLimit);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByText(insideLimit)).toBeTruthy();
    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    expect(acceptButton().disabled).toBe(false);
  });
});
