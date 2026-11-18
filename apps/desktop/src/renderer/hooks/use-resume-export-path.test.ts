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
});
