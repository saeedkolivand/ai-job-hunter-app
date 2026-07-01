import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

import { useUpdater } from './use-updater';

afterEach(() => vi.restoreAllMocks());

function setup() {
  let handler: ((s: unknown) => void) | null = null;
  const check = vi.fn().mockResolvedValue(undefined);
  const download = vi.fn().mockResolvedValue(undefined);
  const install = vi.fn().mockResolvedValue(undefined);
  const client = createMockClient({
    'updater.onStatus': vi.fn((h: (s: unknown) => void) => {
      handler = h;
      return () => {};
    }),
    'updater.check': check,
    'updater.download': download,
    'updater.install': install,
  });
  const hook = renderHook(() => useUpdater(), { wrapper: withProviders(client) });
  return { hook, emit: (s: unknown) => act(() => handler?.(s)), check, download, install };
}

describe('useUpdater', () => {
  it('reflects status events', () => {
    const { hook, emit } = setup();
    expect(hook.result.current.status).toEqual({ state: 'idle' });
    emit({ state: 'available', version: '2.0.0' });
    expect(hook.result.current.status).toEqual({ state: 'available', version: '2.0.0' });
  });

  it('computes download speed and time remaining across progress events', () => {
    let t = 1000;
    vi.spyOn(Date, 'now').mockImplementation(() => t);
    const { hook, emit } = setup();

    t = 1000;
    emit({ state: 'downloading', percent: 10, downloaded: 1_000_000, total: 10_000_000 });
    t = 2000; // 1s later
    emit({ state: 'downloading', percent: 20, downloaded: 3_000_000, total: 10_000_000 });

    expect(hook.result.current.downloadSpeed).toMatch(/\/s$/);
    expect(hook.result.current.timeRemaining).toBeTruthy();
    expect(hook.result.current.totalBytes).toBe(10_000_000);
  });

  it('resets download metrics on completion', () => {
    const { hook, emit } = setup();
    emit({ state: 'downloading', percent: 50, downloaded: 5, total: 10 });
    emit({ state: 'downloaded', version: '2.0.0' });
    expect(hook.result.current.downloadSpeed).toBe('');
    expect(hook.result.current.downloadedBytes).toBe(0);
  });

  it('exposes check/download/install actions wired to the client', () => {
    const { hook, check, download, install } = setup();
    act(() => {
      void hook.result.current.check();
      void hook.result.current.download();
      void hook.result.current.install();
    });
    expect(check).toHaveBeenCalled();
    expect(download).toHaveBeenCalled();
    expect(install).toHaveBeenCalled();
  });
});
