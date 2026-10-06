/**
 * useReferralDraft — `improve()`: replaces the draft with the improved text, never
 * clears it on failure/abort, and never surfaces an abort as an error.
 * Shared mocks live in `draft.test-support.ts`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { render, settle } from './draft.test-helpers';
import { BASE, mockGenerateReferralImprove, resetGenerateMocks } from './draft.test-support';

vi.mock('@/lib/generate', async () => (await import('./draft.test-support')).generateModule);
vi.mock('@ajh/shared/language-detection', async () => {
  return (await import('./draft.test-support')).languageDetectionModule;
});

beforeEach(resetGenerateMocks);

afterEach(() => {
  vi.clearAllMocks();
});

// ── improve() ────────────────────────────────────────────────────────────────

describe('useReferralDraft — improve()', () => {
  it('calls generateReferralImprove with the current draft + instruction and sets the new draft', async () => {
    const { result } = render();

    // First generate a draft.
    await settle(() => result.current.generate());
    expect(result.current.draft).toBe('Hi Bob, I wanted to reach out about the role at Acme.');

    // Now improve it.
    await settle(() => result.current.improve('make it warmer'));

    expect(mockGenerateReferralImprove).toHaveBeenCalledTimes(1);
    expect(mockGenerateReferralImprove).toHaveBeenCalledWith(
      expect.objectContaining({
        draft: 'Hi Bob, I wanted to reach out about the role at Acme.',
        instruction: 'make it warmer',
        personName: 'Bob Chen',
        companyName: 'Acme',
        format: 'linkedin_message',
        model: 'llama3',
      })
    );
    expect(result.current.draft).toBe(
      'Hi Bob! I really wanted to reach out about the role at Acme.'
    );
    expect(result.current.generating).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('does nothing when canGenerate is false (no draft + canUse=false)', async () => {
    const { result } = render({ ...BASE, canUse: false });

    await settle(() => result.current.improve('make it warmer'));

    expect(mockGenerateReferralImprove).not.toHaveBeenCalled();
    expect(result.current.draft).toBe('');
  });

  it('does nothing when draft is empty even if canGenerate is true', async () => {
    const { result } = render();
    // draft starts empty — improve should early-return.

    await settle(() => result.current.improve('make it warmer'));

    expect(mockGenerateReferralImprove).not.toHaveBeenCalled();
  });

  it('replaces the draft with the improved text (streams replacement)', async () => {
    mockGenerateReferralImprove.mockImplementationOnce(
      async ({ onToken }: { onToken?: (tok: string) => void }): Promise<string> => {
        onToken?.('Improved ');
        onToken?.('text.');
        return 'Improved text.';
      }
    );

    const { result } = render();

    // Seed a draft so improve() can act.
    await settle(() => result.current.generate());

    await settle(() => result.current.improve('shorter'));

    expect(result.current.draft).toBe('Improved text.');
  });

  it('surfaces non-abort errors in error state', async () => {
    mockGenerateReferralImprove.mockRejectedValueOnce(new Error('improve failure'));

    const { result } = render();

    // Seed a draft.
    await settle(() => result.current.generate());

    await settle(() => result.current.improve('fix grammar'));

    expect(result.current.error).toBe('improve failure');
    expect(result.current.generating).toBe(false);
  });

  it('passes charLimit=300 for connection_note channel', async () => {
    const { result } = render({ ...BASE, channel: 'connection_note' });

    // Seed a draft first.
    await settle(() => result.current.generate());

    await settle(() => result.current.improve('shorter'));

    expect(mockGenerateReferralImprove).toHaveBeenCalledWith(
      expect.objectContaining({ charLimit: 300, format: 'connection_note' })
    );
  });

  it('abort during improve does NOT set an error', async () => {
    mockGenerateReferralImprove.mockImplementationOnce(async (): Promise<string> => {
      throw new DOMException('The operation was aborted.', 'AbortError');
    });

    const { result } = render();

    // Seed a draft.
    await settle(() => result.current.generate());

    act(() => {
      void result.current.improve('make it warmer');
    });

    await act(async () => {
      result.current.abort();
    });

    expect(result.current.error).toBeNull();
    expect(result.current.generating).toBe(false);
  });

  // ── BUG 3 regression guard: draft must survive failed / aborted improve ────────

  it('draft is preserved (not cleared) when improve() fails with a network error', async () => {
    mockGenerateReferralImprove.mockRejectedValueOnce(new Error('network failure'));

    const { result } = render();

    // Seed a draft.
    await settle(() => result.current.generate());
    const originalDraft = result.current.draft;
    expect(originalDraft).not.toBe('');

    // improve() fails — draft must be restored to the pre-improve value.
    await settle(() => result.current.improve('make it shorter'));

    expect(result.current.draft).toBe(originalDraft);
    expect(result.current.error).toBe('network failure');
    expect(result.current.generating).toBe(false);
  });

  it('draft is preserved (not cleared) when improve() is aborted mid-stream', async () => {
    mockGenerateReferralImprove.mockImplementationOnce(async (): Promise<string> => {
      throw new DOMException('The operation was aborted.', 'AbortError');
    });

    const { result } = render();

    // Seed a draft.
    await settle(() => result.current.generate());
    const originalDraft = result.current.draft;
    expect(originalDraft).not.toBe('');

    // Start improve then abort — draft must survive.
    act(() => {
      void result.current.improve('make it shorter');
    });

    await act(async () => {
      result.current.abort();
    });

    expect(result.current.draft).toBe(originalDraft);
    expect(result.current.error).toBeNull();
    expect(result.current.generating).toBe(false);
  });
});
