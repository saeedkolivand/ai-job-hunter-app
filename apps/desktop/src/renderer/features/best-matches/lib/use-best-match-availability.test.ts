import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { AutopilotAvailabilityCheck, AutopilotBestMatch } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
const notifyInfo = vi.fn();
vi.mock('@ajh/ui', () => ({ useNotification: () => ({ info: notifyInfo }) }));
const mockCheck = vi.fn();
vi.mock('@/services', () => ({ useCheckAvailability: () => ({ mutateAsync: mockCheck }) }));

import { useBestMatchAvailability } from './use-best-match-availability';

const match = (key: string): AutopilotBestMatch => ({
  key,
  title: `Job ${key}`,
  company: 'Acme',
  url: `https://example.com/${key}`,
  score: 80,
  scoreSource: 'combined',
  foundAt: 0,
  sources: [],
});
const verdict = (
  availability: AutopilotAvailabilityCheck['availability']
): AutopilotAvailabilityCheck => ({ availability, source: 'ok', checkedAt: 5 });

describe('useBestMatchAvailability', () => {
  const prepare = vi.fn();
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('prepares an open posting straight away', async () => {
    mockCheck.mockResolvedValue(verdict('open'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    await act(() => result.current.handlePrepare(match('a')));
    expect(mockCheck).toHaveBeenCalledWith({ url: 'https://example.com/a' });
    expect(prepare).toHaveBeenCalledTimes(1);
    expect(result.current.warning).toBeNull();
  });

  it('warns instead of preparing a closed posting, and prepares only after confirmation', async () => {
    mockCheck.mockResolvedValue(verdict('closed'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    await act(() => result.current.handlePrepare(match('a')));
    expect(prepare).not.toHaveBeenCalled();
    expect(result.current.warning?.key).toBe('a');
    expect(result.current.isUnavailable('a')).toBe(true);

    act(() => result.current.confirmPrepareAnyway());
    expect(prepare).toHaveBeenCalledTimes(1);
    expect(result.current.warning).toBeNull();
  });

  it('treats a failed or unknown check as "couldn\'t verify", never as closed', async () => {
    mockCheck.mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce(verdict('unknown'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    await act(() => result.current.handlePrepare(match('a')));
    await act(() => result.current.handlePrepare(match('b')));
    expect(prepare).toHaveBeenCalledTimes(2);
    expect(notifyInfo).toHaveBeenCalledTimes(2);
    expect(result.current.isUnavailable('a')).toBe(false);
    expect(result.current.isUnavailable('b')).toBe(false);
  });

  it('lets the latest Prepare own the warning when checks resolve out of order', async () => {
    let resolveA!: (v: AutopilotAvailabilityCheck) => void;
    mockCheck
      .mockImplementationOnce(() => new Promise((r) => (resolveA = r)))
      .mockResolvedValueOnce(verdict('closed'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    let first!: Promise<void>;
    act(() => {
      first = result.current.handlePrepare(match('a'));
    });
    await act(() => result.current.handlePrepare(match('b')));
    expect(result.current.warning?.key).toBe('b');

    await act(async () => {
      resolveA(verdict('closed'));
      await first;
    });
    expect(result.current.warning?.key).toBe('b');
    act(() => result.current.confirmPrepareAnyway());
    expect(prepare).toHaveBeenCalledTimes(1);
    expect(prepare.mock.calls[0]?.[0].key).toBe('b');
  });

  it('drops a stale open verdict instead of preparing over a newer Prepare', async () => {
    let resolveA!: (v: AutopilotAvailabilityCheck) => void;
    mockCheck
      .mockImplementationOnce(() => new Promise((r) => (resolveA = r)))
      .mockResolvedValueOnce(verdict('open'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    let first!: Promise<void>;
    act(() => {
      first = result.current.handlePrepare(match('a'));
    });
    await act(() => result.current.handlePrepare(match('b')));
    await act(async () => {
      resolveA(verdict('open'));
      await first;
    });
    expect(prepare).toHaveBeenCalledTimes(1);
    expect(prepare.mock.calls[0]?.[0].key).toBe('b');
  });

  it('applies a verdict to the row it was checked for, by key', async () => {
    mockCheck.mockResolvedValue(verdict('open'));
    const { result } = renderHook(() => useBestMatchAvailability(prepare));
    await act(() => result.current.handleCheck(match('a')));
    expect(result.current.withVerdict(match('a')).availability).toBe('open');
    expect(result.current.withVerdict(match('b')).availability).toBeUndefined();
  });
});
