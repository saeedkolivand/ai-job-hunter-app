import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GenerationMeta } from '@ajh/prompts/generate';

import { _registerClient } from '../app-client';
import { createMockClient } from '../mock-client';
import { buildFilename, exportDOCX, exportPDF, exportTXT } from './export';

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

  it('falls back to the modern template for an unknown id', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const exportAndSave = vi.fn().mockResolvedValue('/p');
    _registerClient(createMockClient({ documents: { exportAndSave } }));

    await exportDOCX('Body', 'out.docx', 'resume', meta, 'does-not-exist' as never);
    expect(warn).toHaveBeenCalled();
    expect(exportAndSave).toHaveBeenCalledWith(expect.objectContaining({ templateId: 'modern' }));
  });

  it('throws on empty content', async () => {
    _registerClient(createMockClient());
    await expect(exportDOCX('', 'out.docx')).rejects.toThrow(/empty document/);
  });
});
