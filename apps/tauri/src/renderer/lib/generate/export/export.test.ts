import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GenerationMeta } from '@ajh/prompts/generate';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import { buildFilename, exportDOCX, exportPDF, exportTXT, renderDocumentPreview } from './export';

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

describe('renderDocumentPreview (#24)', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders via renderPreviewImages (no save) and returns SVG page strings', async () => {
    const renderPreviewImages = vi
      .fn()
      .mockResolvedValue({ pages: ['<svg>p1</svg>', '<svg>p2</svg>'], mimeType: 'image/svg+xml' });
    _registerClient(createMockClient({ documents: { renderPreviewImages } }));

    const pages = await renderDocumentPreview('Resume body', 'resume', meta, 'modern');
    expect(renderPreviewImages).toHaveBeenCalledWith(
      expect.objectContaining({ documentType: 'resume', templateId: 'modern' })
    );
    expect(pages).toEqual(['<svg>p1</svg>', '<svg>p2</svg>']);
  });

  it('extracts the cover-letter section before rendering', async () => {
    const renderPreviewImages = vi.fn().mockResolvedValue({ pages: [], mimeType: 'image/svg+xml' });
    _registerClient(createMockClient({ documents: { renderPreviewImages } }));

    const raw = 'Resume context\n### COMPLETE COVER LETTER ###\nDear Hiring Team, ...';
    await renderDocumentPreview(raw, 'cover-letter', meta, 'classic');
    const arg = renderPreviewImages.mock.calls[0]?.[0] as { text: string };
    expect(arg.text.startsWith('Dear Hiring Team')).toBe(true);
  });

  it('throws on empty text and on an unknown template', async () => {
    _registerClient(createMockClient());
    await expect(renderDocumentPreview('   ', 'resume', meta, 'modern')).rejects.toThrow(/empty/);
    await expect(renderDocumentPreview('Body', 'resume', meta, 'nope' as never)).rejects.toThrow(
      /Unknown export template/
    );
  });

  it('escapes raw & in SVG hrefs (Typst query-param bug) without double-escaping existing entities', async () => {
    const rawSvg = '<svg><a href="https://x.com/u?a=1&lipi=2&licu=3"/></svg>';
    const renderPreviewImages = vi
      .fn()
      .mockResolvedValue({ pages: [rawSvg], mimeType: 'image/svg+xml' });
    _registerClient(createMockClient({ documents: { renderPreviewImages } }));

    const pages = await renderDocumentPreview('Resume body', 'resume', meta, 'modern');
    // raw & must be escaped
    expect(pages[0]).not.toMatch(/&lipi=/);
    expect(pages[0]).not.toMatch(/&licu=/);
    expect(pages[0]).toContain('&amp;lipi=');
    expect(pages[0]).toContain('&amp;licu=');
    // pre-existing &amp; must NOT be double-escaped
    const withEntity = '<svg><a href="https://x.com/u?q=a&amp;b=c"/></svg>';
    const renderPreviewImages2 = vi
      .fn()
      .mockResolvedValue({ pages: [withEntity], mimeType: 'image/svg+xml' });
    _registerClient(createMockClient({ documents: { renderPreviewImages: renderPreviewImages2 } }));
    const pages2 = await renderDocumentPreview('Resume body', 'resume', meta, 'modern');
    expect(pages2[0]).not.toContain('&amp;amp;');
    expect(pages2[0]).toContain('&amp;b=c');
  });
});
