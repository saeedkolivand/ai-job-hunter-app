import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { exportPDF, exportTXT } from '@/lib/generate';

import { useGenerationExport } from './useGenerationExport';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', () => ({ useNotification: () => ({ error: vi.fn() }) }));
vi.mock('@/lib/generate', () => ({
  buildFilename: () => 'cv.pdf',
  exportDOCX: vi.fn(),
  exportPDF: vi.fn(),
  exportTXT: vi.fn(),
  resolveMarket: () => 'intl',
}));

const meta = {} as Parameters<typeof useGenerationExport>[0]['meta'];

describe('useGenerationExport', () => {
  it('ignores a stale PDF résumé export that resolves after a newer TXT export', async () => {
    let resolveA: (p: string) => void = () => {};
    vi.mocked(exportPDF).mockImplementationOnce(() => new Promise<string>((r) => (resolveA = r)));
    const { result } = renderHook(() =>
      useGenerationExport({ meta, resumeDraft: 'cv', coverDraft: 'letter' })
    );
    let a: Promise<void> = Promise.resolve();
    act(() => {
      a = result.current.doExport('resume');
    });
    act(() => result.current.setFormat('txt'));
    await act(() => result.current.doExport('resume'));
    expect(exportTXT).toHaveBeenCalledOnce();
    await act(async () => {
      resolveA('/out/stale.pdf');
      await a;
    });
    expect(result.current.resumeExportPath).toBeNull();
  });
});
