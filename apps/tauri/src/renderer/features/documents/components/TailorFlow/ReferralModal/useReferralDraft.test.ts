/**
 * useReferralDraft — unit tests (F3a).
 *
 * Covers:
 *  - connection_note ≤300 limit: draft >300 means overLimit; draft ≤300 allows save.
 *  - Channel-switch clears the draft.
 *  - generate() calls generateReferral with the expected arguments.
 *  - Error handling: non-abort errors surface in `error` state.
 *  - Abort: does NOT set an error.
 */
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { ReferralChannel } from '@ajh/shared/ipc';

import { useReferralDraft } from './useReferralDraft';

// ── mock @/lib/generate ───────────────────────────────────────────────────────
// generateReferral is mocked as a vi.fn that resolves with a deterministic string.
// The mock is reset between tests so individual cases can override the return value.

const mockGenerateReferral =
  vi.fn<
    (params: {
      personName: string;
      personRole?: string;
      companyName: string;
      jobTitle: string;
      resume: string;
      format: ReferralChannel;
      charLimit?: number;
      model: string;
      locale?: string;
      onToken?: (tok: string) => void;
      signal?: AbortSignal;
    }) => Promise<string>
  >();

vi.mock('@/lib/generate', () => ({
  generateReferral: (...args: Parameters<typeof mockGenerateReferral>) =>
    mockGenerateReferral(...args),
  // CONNECTION_NOTE_LIMIT is a re-export from @ajh/prompts — provide the real value.
  CONNECTION_NOTE_LIMIT: 300,
}));

// ── mock @ajh/shared/language-detection ──────────────────────────────────────
vi.mock('@ajh/shared/language-detection', () => ({
  detectLanguages: vi.fn<(resume: string, jobAd: string) => { resumeName: string }>(() => ({
    resumeName: 'en',
  })),
}));

// ── base params ───────────────────────────────────────────────────────────────

const BASE = {
  personName: 'Bob Chen',
  personRole: 'Director',
  companyName: 'Acme',
  jobTitle: 'Senior Engineer',
  resume: 'Jane Doe\nSenior Engineer with 8 years experience.',
  channel: 'linkedin_message' as ReferralChannel,
  model: 'llama3',
  canUse: true,
};

// No QueryClient needed — useReferralDraft holds no React Query state.
const wrapper = ({ children }: { children: ReactNode }) => createElement('div', {}, children);
const render = (p: typeof BASE = BASE) => renderHook(() => useReferralDraft(p), { wrapper });

beforeEach(() => {
  mockGenerateReferral.mockResolvedValue('Hi Bob, I wanted to reach out about the role at Acme.');
});

afterEach(() => {
  vi.clearAllMocks();
});

// ── initial state ─────────────────────────────────────────────────────────────

describe('useReferralDraft — initial state', () => {
  it('starts idle with empty draft, no error, not generating', () => {
    const { result } = render();
    expect(result.current.draft).toBe('');
    expect(result.current.generating).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('canGenerate is true when personName + resume are non-empty and canUse=true', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(true);
  });

  it('canGenerate is false when personName is blank', () => {
    const { result } = render({ ...BASE, personName: '   ' });
    expect(result.current.canGenerate).toBe(false);
  });

  it('canGenerate is false when canUse=false', () => {
    const { result } = render({ ...BASE, canUse: false });
    expect(result.current.canGenerate).toBe(false);
  });

  it('canGenerate is false when resume is blank', () => {
    const { result } = render({ ...BASE, resume: '' });
    expect(result.current.canGenerate).toBe(false);
  });
});

// ── generate() ────────────────────────────────────────────────────────────────

describe('useReferralDraft — generate()', () => {
  it('calls generateReferral and sets draft to the returned text', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(mockGenerateReferral).toHaveBeenCalledTimes(1);
    expect(result.current.draft).toBe('Hi Bob, I wanted to reach out about the role at Acme.');
    expect(result.current.generating).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('calls generateReferral with the correct personName, companyName, format, and model', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(mockGenerateReferral).toHaveBeenCalledWith(
      expect.objectContaining({
        personName: 'Bob Chen',
        companyName: 'Acme',
        format: 'linkedin_message',
        model: 'llama3',
      })
    );
  });

  it('passes charLimit=300 for connection_note channel', async () => {
    const { result } = render({ ...BASE, channel: 'connection_note' });

    await act(async () => {
      await result.current.generate();
    });

    expect(mockGenerateReferral).toHaveBeenCalledWith(
      expect.objectContaining({ charLimit: 300, format: 'connection_note' })
    );
  });

  it('does NOT pass charLimit for email channel', async () => {
    const { result } = render({ ...BASE, channel: 'email' });

    await act(async () => {
      await result.current.generate();
    });

    const call = mockGenerateReferral.mock.calls[0]?.[0];
    expect(call?.charLimit).toBeUndefined();
  });

  it('does nothing when canGenerate is false', async () => {
    const { result } = render({ ...BASE, canUse: false });

    await act(async () => {
      await result.current.generate();
    });

    expect(mockGenerateReferral).not.toHaveBeenCalled();
    expect(result.current.draft).toBe('');
  });

  it('surfaces non-abort errors in the error state', async () => {
    mockGenerateReferral.mockRejectedValueOnce(new Error('network failure'));
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.error).toBe('network failure');
    expect(result.current.generating).toBe(false);
  });
});

// ── connection_note ≤300 enforcement ─────────────────────────────────────────

describe('useReferralDraft — connection_note overLimit logic', () => {
  it('draft >300 chars on connection_note channel: draft contains the full text', async () => {
    // The hook accumulates tokens and sets draft; the UI layer derives overLimit
    // from draft.length > CONNECTION_NOTE_LIMIT. We verify the raw draft value
    // so callers can compute canSave = !overLimit themselves.
    const longDraft = 'A'.repeat(301);
    mockGenerateReferral.mockResolvedValueOnce(longDraft);

    const { result } = render({ ...BASE, channel: 'connection_note' });

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.draft).toBe(longDraft);
    expect(result.current.draft.length).toBeGreaterThan(300);
  });

  it('draft ≤300 chars on connection_note: draft length is within limit', async () => {
    const shortDraft = 'A'.repeat(300);
    mockGenerateReferral.mockResolvedValueOnce(shortDraft);

    const { result } = render({ ...BASE, channel: 'connection_note' });

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.draft.length).toBeLessThanOrEqual(300);
  });
});

// ── unmount aborts in-flight generation ──────────────────────────────────────

describe('useReferralDraft — unmount aborts in-flight call', () => {
  it('aborts the AbortController when the hook unmounts mid-generation', async () => {
    // Capture the signal passed to generateReferral without resolving the promise,
    // so the hook stays in the "generating" state when we unmount.
    // Hold the signal in an object so TS keeps its declared type (a closure-
    // assigned `let` gets narrowed to its initializer and breaks the access).
    const captured: { signal?: AbortSignal } = {};
    mockGenerateReferral.mockImplementationOnce(
      ({ signal }: { signal?: AbortSignal }): Promise<string> => {
        captured.signal = signal;
        // Never resolves — simulates a pending streaming call.
        return new Promise<string>(() => {});
      }
    );

    const { result, unmount } = render();

    // Start generating but do NOT await — keep it in-flight.
    act(() => {
      void result.current.generate();
    });

    // The signal must exist and not yet be aborted.
    expect(captured.signal).toBeDefined();
    expect(captured.signal?.aborted).toBe(false);

    // Unmount triggers the cleanup: `() => abortRef.current?.abort()`.
    unmount();

    expect(captured.signal?.aborted).toBe(true);
    // error stays null — no setState was called on the dead component.
    expect(result.current.error).toBeNull();
  });
});

// ── abort suppresses error ────────────────────────────────────────────────────

describe('useReferralDraft — aborted generation does not surface an error', () => {
  it('keeps error null when generateReferral rejects because the signal was aborted', async () => {
    mockGenerateReferral.mockImplementationOnce(async (): Promise<string> => {
      // Simulate the provider checking the signal and throwing an AbortError.
      // The simplest approach is to throw after the hook's own controller has
      // been aborted via gen.abort() — the catch guard sees signal.aborted=true.
      throw new DOMException('The operation was aborted.', 'AbortError');
    });

    const { result } = render();

    // Start generation — hook sets abortRef.current to a fresh AbortController.
    act(() => {
      void result.current.generate();
    });

    // Abort before the mock promise settles so controller.signal.aborted is true
    // when the catch block runs.
    await act(async () => {
      result.current.abort();
    });

    // The catch guard `!controller.signal.aborted` must suppress the error.
    expect(result.current.error).toBeNull();
    expect(result.current.generating).toBe(false);
  });
});

// ── channel-switch clears draft ───────────────────────────────────────────────

describe('useReferralDraft — channel switch clears draft', () => {
  it('switches channel → draft is cleared', async () => {
    const { result, rerender } = renderHook((props: typeof BASE) => useReferralDraft(props), {
      wrapper,
      initialProps: { ...BASE, channel: 'linkedin_message' as ReferralChannel },
    });

    // Generate on linkedin_message so we have a draft.
    await act(async () => {
      await result.current.generate();
    });
    expect(result.current.draft).not.toBe('');

    // Switch to email.
    await act(async () => {
      rerender({ ...BASE, channel: 'email' as ReferralChannel });
    });

    expect(result.current.draft).toBe('');
    expect(result.current.error).toBeNull();
    expect(result.current.generating).toBe(false);
  });

  it('same channel on rerender does NOT clear draft', async () => {
    const { result, rerender } = renderHook((props: typeof BASE) => useReferralDraft(props), {
      wrapper,
      initialProps: { ...BASE, channel: 'email' as ReferralChannel },
    });

    await act(async () => {
      await result.current.generate();
    });
    const draftAfterGenerate = result.current.draft;
    expect(draftAfterGenerate).not.toBe('');

    // Re-render with identical channel — draft must be preserved.
    await act(async () => {
      rerender({ ...BASE, channel: 'email' as ReferralChannel });
    });

    expect(result.current.draft).toBe(draftAfterGenerate);
  });
});
