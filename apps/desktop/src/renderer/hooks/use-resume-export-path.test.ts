import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useResumeExportPath } from './use-resume-export-path';

describe('useResumeExportPath', () => {
  it('records the saved path of a résumé export', async () => {
    const { result } = renderHook(() => useResumeExportPath());
    await act(() => result.current.track(true, async () => '/out/cv.pdf'));
    expect(result.current.path).toBe('/out/cv.pdf');
  });

  it('clears the path when a new résumé export starts, and keeps it cleared on error', async () => {
    const { result } = renderHook(() => useResumeExportPath());
    await act(() => result.current.track(true, async () => '/out/cv.pdf'));
    await act(async () => {
      await expect(
        result.current.track(true, async () => {
          throw new Error('boom');
        })
      ).rejects.toThrow('boom');
    });
    expect(result.current.path).toBeNull();
  });

  it('leaves the path alone for a cover letter and never sets one for TXT', async () => {
    const { result } = renderHook(() => useResumeExportPath());
    await act(() => result.current.track(true, async () => '/out/cv.pdf'));
    await act(() => result.current.track(false, async () => '/out/letter.pdf'));
    expect(result.current.path).toBe('/out/cv.pdf');
    await act(() => result.current.track(true, async () => undefined));
    expect(result.current.path).toBeNull();
  });

  it('ignores a stale PDF export that resolves after a newer TXT export', async () => {
    const { result } = renderHook(() => useResumeExportPath());
    let resolveA: (p: string) => void = () => {};
    let a: Promise<void> = Promise.resolve();
    act(() => {
      a = result.current.track(true, () => new Promise<string>((r) => (resolveA = r)));
    });
    await act(() => result.current.track(true, async () => undefined));
    await act(async () => {
      resolveA('/out/stale.pdf');
      await a;
    });
    expect(result.current.path).toBeNull();
  });

  it('does not clobber a newer export when a superseded one errors', async () => {
    const { result } = renderHook(() => useResumeExportPath());
    let rejectA: (e: Error) => void = () => {};
    let a: Promise<void> = Promise.resolve();
    act(() => {
      a = result.current.track(true, () => new Promise<string>((_, rej) => (rejectA = rej)));
    });
    await act(() => result.current.track(true, async () => '/out/new.pdf'));
    await act(async () => {
      rejectA(new Error('boom'));
      await expect(a).rejects.toThrow('boom');
    });
    expect(result.current.path).toBe('/out/new.pdf');
  });
});
