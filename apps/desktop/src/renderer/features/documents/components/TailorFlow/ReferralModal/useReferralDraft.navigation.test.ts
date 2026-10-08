/** Rule 16: a referral draft survives the modal unmounting mid-stream. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { render, settle } from './draft.test-helpers';
import { BASE, mockGenerateReferral, resetGenerateMocks } from './draft.test-support';

vi.mock('@/lib/generate', async () => (await import('./draft.test-support')).generateModule);
vi.mock('@ajh/shared/language-detection', async () => {
  return (await import('./draft.test-support')).languageDetectionModule;
});

beforeEach(resetGenerateMocks);

describe('useReferralDraft — survives unmount', () => {
  it('keeps streaming after unmount and shows it in flight on remount', async () => {
    let onToken: (t: string) => void = () => {};
    let finish: (t: string) => void = () => {};
    let signal: AbortSignal | undefined;
    mockGenerateReferral.mockImplementation((p) => {
      onToken = p.onToken ?? onToken;
      signal = p.signal;
      return new Promise<string>((res) => {
        finish = res;
      });
    });

    const first = render();
    let pending: Promise<void> = Promise.resolve();
    await act(async () => {
      pending = first.result.current.generate();
    });
    first.unmount();
    expect(signal?.aborted).toBe(false);

    await act(async () => onToken('Hi '));
    const second = render();
    expect(second.result.current.generating).toBe(true);
    expect(second.result.current.draft).toBe('Hi ');

    await act(async () => {
      finish('Hi Bob');
      await pending;
    });
    expect(second.result.current.generating).toBe(false);
    expect(second.result.current.draft).toBe('Hi Bob');
  });

  it('shows the finished draft on remount when it completed while unmounted', async () => {
    let finish: (t: string) => void = () => {};
    mockGenerateReferral.mockImplementation(
      () =>
        new Promise<string>((res) => {
          finish = res;
        })
    );
    const first = render();
    let pending: Promise<void> = Promise.resolve();
    await act(async () => {
      pending = first.result.current.generate();
    });
    first.unmount();
    await settle(async () => {
      finish('Done while away');
      await pending;
    });

    const second = render();
    expect(second.result.current.draft).toBe('Done while away');
    expect(second.result.current.generating).toBe(false);
  });

  it('does not show another job’s draft', async () => {
    const first = render();
    await settle(() => first.result.current.generate());
    expect(first.result.current.draft).not.toBe('');
    const other = render({ ...BASE, jobUrl: 'https://example.com/job/2' });
    expect(other.result.current.draft).toBe('');
  });
});
