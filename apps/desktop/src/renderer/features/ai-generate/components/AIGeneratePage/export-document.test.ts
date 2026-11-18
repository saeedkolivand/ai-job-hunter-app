import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { exportDOCX, exportPDF, exportTXT } from '@/lib/generate';

import {
  exportActiveDocument,
  type ExportDocumentState,
  useActiveDocumentExport,
} from './export-document';

vi.mock('@/lib/generate', () => ({
  buildFilename: () => 'file',
  exportPDF: vi.fn(async () => '/out/file.pdf'),
  exportDOCX: vi.fn(async () => '/out/file.docx'),
  exportTXT: vi.fn(),
  resolveMarket: () => 'intl',
}));

const state = (activeOut: 'resume' | 'cover'): ExportDocumentState => ({
  activeOut,
  resumeOut: 'resume text',
  coverOut: 'cover text',
  meta: null,
  locale: 'en',
  templateId: 'classic',
  atsMode: false,
  accent: undefined,
  letterLayoutId: undefined,
});

/** Runs the export with a spy `track`; resolves what the exporter produced. */
async function run(activeOut: 'resume' | 'cover', fmt: 'pdf' | 'docx' | 'txt') {
  let isResume: boolean | undefined;
  let saved: string | void = undefined;
  await exportActiveDocument(state(activeOut), fmt, async (resume, exec) => {
    isResume = resume;
    saved = await exec();
  });
  return { isResume, saved };
}

describe('exportActiveDocument', () => {
  it('reports a résumé PDF/DOCX export and its saved path to track', async () => {
    expect(await run('resume', 'pdf')).toEqual({ isResume: true, saved: '/out/file.pdf' });
    expect(await run('resume', 'docx')).toEqual({ isResume: true, saved: '/out/file.docx' });
  });

  it('resolves no path for TXT', async () => {
    expect((await run('resume', 'txt')).saved).toBeUndefined();
  });

  it('flags a cover letter as not a résumé', async () => {
    expect((await run('cover', 'pdf')).isResume).toBe(false);
  });
});

describe('useActiveDocumentExport', () => {
  it('exposes the saved path after a résumé export, cleared by TXT, never set for a cover letter', async () => {
    const { result } = renderHook(() => useActiveDocumentExport(state('resume'), false));
    expect(result.current.atsViewPath).toBeNull();
    await act(() => result.current.doExport('pdf'));
    expect(result.current.atsViewPath).toBe('/out/file.pdf');
    await act(() => result.current.doExport('txt'));
    expect(result.current.atsViewPath).toBeNull();

    const cover = renderHook(() => useActiveDocumentExport(state('cover'), false));
    await act(() => cover.result.current.doExport('pdf'));
    expect(cover.result.current.atsViewPath).toBeNull();
  });
});

describe('useActiveDocumentExport — generation id', () => {
  beforeEach(() => vi.clearAllMocks());

  // exporter(text, name, type, meta, templateId, atsMode, locale, accent, letterLayoutId, generationId)
  it.each([
    ['pdf', exportPDF],
    ['docx', exportDOCX],
  ] as const)(
    'passes the saved generation id as the final %s exporter argument',
    async (fmt, exporter) => {
      const { result } = renderHook(() =>
        useActiveDocumentExport(state('resume'), false, 'gen-42')
      );
      await act(() => result.current.doExport(fmt));
      expect(exporter).toHaveBeenCalledTimes(1);
      expect(vi.mocked(exporter).mock.calls[0]?.[9]).toBe('gen-42');
    }
  );

  it('passes no id when the save failed (none captured)', async () => {
    const { result } = renderHook(() => useActiveDocumentExport(state('resume'), false));
    await act(() => result.current.doExport('pdf'));
    expect(vi.mocked(exportPDF).mock.calls[0]?.[9]).toBeUndefined();
  });

  it('TXT and blocked exports call neither PDF nor DOCX exporter', async () => {
    const open = renderHook(() => useActiveDocumentExport(state('resume'), false, 'gen-42'));
    await act(() => open.result.current.doExport('txt'));
    const blocked = renderHook(() => useActiveDocumentExport(state('resume'), true, 'gen-42'));
    await act(() => blocked.result.current.doExport('pdf'));
    await act(() => blocked.result.current.doExport('docx'));
    expect(exportPDF).not.toHaveBeenCalled();
    expect(exportDOCX).not.toHaveBeenCalled();
    expect(exportTXT).toHaveBeenCalledTimes(1);
  });
});
