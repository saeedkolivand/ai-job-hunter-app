import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GenerationMeta } from '@ajh/prompts/generate';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import { buildFilename, exportDOCX, exportPDF, exportTXT, renderPdfPreview } from './export';

const meta: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'Jöhn Doe!',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme & Co',
  targetLanguage: 'en',
  topRequirements: [],
};

describe('buildFilename', () => {
  it('sanitises name, role and company and appends the extension', () => {
    expect(buildFilename(meta, 'resume', 'pdf')).toBe(
      'John-Doe-Senior-Engineer-Acme-Co-resume.pdf'
    );
  });

  it('falls back to placeholders for empty fields', () => {
    const blank = { ...meta, candidateName: '', jobTitle: '', companyName: '' };
    expect(buildFilename(blank, 'cover-letter', 'docx')).toBe(
      'Candidate-Role-Company-cover-letter.docx'
    );
  });
});

describe('exportTXT', () => {
  beforeEach(() => {
    URL.createObjectURL = vi.fn(() => 'blob:mock');
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(() => vi.restoreAllMocks());

  it('strips bold markers and triggers a download', () => {
    exportTXT('Hello **world**', 'out.txt');
    expect(URL.createObjectURL).toHaveBeenCalledOnce();
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:mock');
  });

  it('throws on empty content', () => {
    expect(() => exportTXT('   ', 'out.txt')).toThrow(/empty document/);
  });
});

describe('exportDOCX / exportPDF', () => {
  afterEach(() => vi.restoreAllMocks());

  it('routes resume text to the backend exporter', async () => {
    const exportAndSave = vi.fn().mockResolvedValue('/path/out.docx');
    _registerClient(createMockClient({ documents: { exportAndSave } }));

    await exportDOCX('Resume body', 'out.docx', 'resume', meta, 'modern');
    expect(exportAndSave).toHaveBeenCalledWith(
      expect.objectContaining({ format: 'docx', documentType: 'resume', templateId: 'modern' })
    );
  });

  it('extracts the cover-letter section before exporting', async () => {
    const exportAndSave = vi.fn().mockResolvedValue('/path/out.pdf');
    _registerClient(createMockClient({ documents: { exportAndSave } }));

    const raw = 'Resume context\n### COMPLETE COVER LETTER ###\nDear Hiring Team, ...';
    await exportPDF(raw, 'out.pdf', 'cover-letter', meta, 'classic');
    const arg = exportAndSave.mock.calls[0]?.[0] as { text: string; format: string };
    expect(arg.format).toBe('pdf');
    expect(arg.text.startsWith('Dear Hiring Team')).toBe(true);
  });

  it('throws on an unknown template id instead of silently falling back', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const exportAndSave = vi.fn().mockResolvedValue('/p');
    _registerClient(createMockClient({ documents: { exportAndSave } }));

    // A wrong-template export is indistinguishable from a correct one, so the
    // unknown id surfaces as an error rather than quietly swapping in "modern".
    await expect(
      exportDOCX('Body', 'out.docx', 'resume', meta, 'does-not-exist' as never)
    ).rejects.toThrow(/Unknown export template/);
    expect(exportAndSave).not.toHaveBeenCalled();
  });

  it('throws on empty content', async () => {
    _registerClient(createMockClient());
    await expect(exportDOCX('', 'out.docx')).rejects.toThrow(/empty document/);
  });
});

describe('renderPdfPreview (#24)', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders via exportDocument (no save) and returns the PDF bytes', async () => {
    const exportDocument = vi
      .fn()
      .mockResolvedValue({ data: [1, 2, 3], mimeType: 'application/pdf' });
    _registerClient(createMockClient({ documents: { exportDocument } }));

    const bytes = await renderPdfPreview('Resume body', 'resume', meta, 'modern');
    expect(exportDocument).toHaveBeenCalledWith(
      expect.objectContaining({ format: 'pdf', documentType: 'resume', templateId: 'modern' })
    );
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(Array.from(bytes)).toEqual([1, 2, 3]);
  });

  it('extracts the cover-letter section before rendering', async () => {
    const exportDocument = vi.fn().mockResolvedValue({ data: [], mimeType: 'application/pdf' });
    _registerClient(createMockClient({ documents: { exportDocument } }));

    const raw = 'Resume context\n### COMPLETE COVER LETTER ###\nDear Hiring Team, ...';
    await renderPdfPreview(raw, 'cover-letter', meta, 'classic');
    const arg = exportDocument.mock.calls[0]?.[0] as { text: string };
    expect(arg.text.startsWith('Dear Hiring Team')).toBe(true);
  });

  it('throws on empty text and on an unknown template', async () => {
    _registerClient(createMockClient());
    await expect(renderPdfPreview('   ', 'resume', meta, 'modern')).rejects.toThrow(/empty/);
    await expect(renderPdfPreview('Body', 'resume', meta, 'nope' as never)).rejects.toThrow(
      /Unknown export template/
    );
  });
});
