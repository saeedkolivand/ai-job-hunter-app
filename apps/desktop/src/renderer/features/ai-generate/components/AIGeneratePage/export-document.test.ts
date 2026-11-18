import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

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
