import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useTailorOutputActions } from './useTailorOutputActions';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ error: vi.fn() }),
}));
vi.mock('@/lib/generate', () => ({
  buildFilename: () => 'file',
  exportPDF: vi.fn(async () => '/out/file.pdf'),
  exportDOCX: vi.fn(async () => '/out/file.docx'),
  exportTXT: vi.fn(),
}));

const setup = (activeOut: 'resume' | 'cover') =>
  renderHook(() =>
    useTailorOutputActions({
      output: 'text',
      activeOut,
      meta: null,
      market: 'intl',
      templateId: 'classic',
      atsMode: false,
    })
  );

describe('useTailorOutputActions — atsViewPath', () => {
  it('is set after a résumé PDF export and cleared by a TXT export', async () => {
    const { result } = setup('resume');
    expect(result.current.atsViewPath).toBeNull();
    await act(() => result.current.exportAs('pdf'));
    expect(result.current.atsViewPath).toBe('/out/file.pdf');
    await act(() => result.current.exportAs('txt'));
    expect(result.current.atsViewPath).toBeNull();
  });

  it('is never set by a cover-letter export', async () => {
    const { result } = setup('cover');
    await act(() => result.current.exportAs('pdf'));
    expect(result.current.atsViewPath).toBeNull();
  });
});
