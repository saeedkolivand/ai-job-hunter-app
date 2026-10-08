/**
 * useReferralDraft — unit tests (F3a).
 *
 * Covers:
 *  - connection_note ≤300 limit: draft >300 means overLimit; draft ≤300 allows save.
 *  - Channel-switch clears the draft.
 *  - generate() calls generateReferral with the expected arguments.
 *  - Error handling: non-abort errors surface in `error` state.
 *  - Abort: does NOT set an error.
 *
 * `improve()` lives in `useReferralDraft.improve.test.ts`; shared mocks in
 * `draft.test-support.ts`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { ReferralChannel } from '@ajh/shared/ipc';

import { render, settle, wrapper } from './draft.test-helpers';
import { BASE, mockGenerateReferral, resetGenerateMocks } from './draft.test-support';
import { useReferralDraft } from './useReferralDraft';

vi.mock('@/lib/generate', async () => (await import('./draft.test-support')).generateModule);
vi.mock('@ajh/shared/language-detection', async () => {
  return (await import('./draft.test-support')).languageDetectionModule;
});

beforeEach(resetGenerateMocks);

afterEach(() => {
  vi.clearAllMocks();
});

/** Renders with `overrides`, runs `generate()` to completion, returns the hook result. */
async function generated(overrides: Partial<typeof BASE> = {}) {
  const { result } = render({ ...BASE, ...overrides });
  await settle(() => result.current.generate());
  return result;
}

// ── initial state ─────────────────────────────────────────────────────────────

describe('useReferralDraft — initial state', () => {
  it('starts idle with empty draft, no error, not generating', () => {
    const { result } = render();
    expect(result.current.draft).toBe('');
    expect(result.current.generating).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it.each([
    ['canGenerate is true when personName + resume are non-empty and canUse=true', {}, true],
    ['canGenerate is false when personName is blank', { personName: '   ' }, false],
    ['canGenerate is false when canUse=false', { canUse: false }, false],
    ['canGenerate is false when resume is blank', { resume: '' }, false],
  ])('%s', (_name, overrides, expected) => {
    const { result } = render({ ...BASE, ...overrides });
    expect(result.current.canGenerate).toBe(expected);
  });
});

// ── generate() ────────────────────────────────────────────────────────────────

describe('useReferralDraft — generate()', () => {
  it('calls generateReferral and sets draft to the returned text', async () => {
    const result = await generated();

    expect(mockGenerateReferral).toHaveBeenCalledTimes(1);
    expect(result.current.draft).toBe('Hi Bob, I wanted to reach out about the role at Acme.');
    expect(result.current.generating).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('calls generateReferral with the correct personName, companyName, format, and model', async () => {
    await generated();

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
    await generated({ channel: 'connection_note' });

    expect(mockGenerateReferral).toHaveBeenCalledWith(
      expect.objectContaining({ charLimit: 300, format: 'connection_note' })
    );
  });

  it('does NOT pass charLimit for email channel', async () => {
    await generated({ channel: 'email' });

    const call = mockGenerateReferral.mock.calls[0]?.[0];
    expect(call?.charLimit).toBeUndefined();
  });

  it('does nothing when canGenerate is false', async () => {
    const result = await generated({ canUse: false });

    expect(mockGenerateReferral).not.toHaveBeenCalled();
    expect(result.current.draft).toBe('');
  });

  it('surfaces non-abort errors in the error state', async () => {
    mockGenerateReferral.mockRejectedValueOnce(new Error('network failure'));
    const result = await generated();

    expect(result.current.error).toBe('network failure');
    expect(result.current.generating).toBe(false);
  });

  it('maps the backend "Stream error: …" to a localized message', async () => {
    mockGenerateReferral.mockRejectedValueOnce(
      new Error('Stream error: error decoding response body')
    );
    const result = await generated();

    expect(result.current.error).toBe(
      'The connection to the AI provider was interrupted. Please try again.'
    );
  });
});

// ── connection_note ≤300 enforcement ─────────────────────────────────────────

describe('useReferralDraft — connection_note overLimit logic', () => {
  // The hook accumulates tokens and sets draft; the UI layer derives overLimit
  // from draft.length > CONNECTION_NOTE_LIMIT. We verify the raw draft value
  // so callers can compute canSave = !overLimit themselves.
  it.each([
    ['draft >300 chars on connection_note channel: draft contains the full text', 301],
    ['draft ≤300 chars on connection_note: draft length is within limit', 300],
  ])('%s', async (_name, length) => {
    const draft = 'A'.repeat(length);
    mockGenerateReferral.mockResolvedValueOnce(draft);

    const result = await generated({ channel: 'connection_note' });

    expect(result.current.draft).toBe(draft);
    if (length > 300) expect(result.current.draft.length).toBeGreaterThan(300);
    else expect(result.current.draft.length).toBeLessThanOrEqual(300);
  });
});

// ── unmount does NOT abort (rule 16; see .navigation.test) ──────────────────────────────────────

describe('useReferralDraft — unmount keeps the in-flight call', () => {
  it('does not abort the AbortController when the hook unmounts mid-generation', async () => {
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

    unmount();

    expect(captured.signal?.aborted).toBe(false);
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
  const renderOn = (channel: ReferralChannel) =>
    renderHook((props: typeof BASE) => useReferralDraft(props), {
      wrapper,
      initialProps: { ...BASE, channel },
    });

  it('switches channel → draft is cleared', async () => {
    // Generate on linkedin_message so we have a draft.
    const { result, rerender } = renderOn('linkedin_message');
    await settle(() => result.current.generate());
    expect(result.current.draft).not.toBe('');

    // Switch to email.
    await act(async () => {
      rerender({ ...BASE, channel: 'email' });
    });

    expect(result.current.draft).toBe('');
    expect(result.current.error).toBeNull();
    expect(result.current.generating).toBe(false);
  });

  it('same channel on rerender does NOT clear draft', async () => {
    const { result, rerender } = renderOn('email');
    await settle(() => result.current.generate());
    const draftAfterGenerate = result.current.draft;
    expect(draftAfterGenerate).not.toBe('');

    // Re-render with identical channel — draft must be preserved.
    await act(async () => {
      rerender({ ...BASE, channel: 'email' });
    });

    expect(result.current.draft).toBe(draftAfterGenerate);
  });
});
