import { describe, expect, it } from 'vitest';

import { buildCandidates, parseDocumentsResourceData } from './resource';

const EMPTY = { generation: null, documents: [] };

describe('parseDocumentsResourceData', () => {
  it('parses a full payload', () => {
    const data = parseDocumentsResourceData({
      generation: {
        hasResume: true,
        hasCoverLetter: true,
        jobTitle: 'Backend Engineer',
        company: 'Acme',
        targetLanguage: 'en',
        updatedAt: 123,
      },
      documents: [
        { id: 'doc-1', name: 'My Résumé.pdf', updatedAt: 1 },
        { id: 'doc-2', name: 'Old Résumé.pdf' },
      ],
    });
    expect(data.generation).toEqual({
      hasResume: true,
      hasCoverLetter: true,
      jobTitle: 'Backend Engineer',
      company: 'Acme',
    });
    expect(data.documents).toEqual([
      { id: 'doc-1', name: 'My Résumé.pdf' },
      { id: 'doc-2', name: 'Old Résumé.pdf' },
    ]);
  });

  it('degrades to empty on malformed/missing data (never throws)', () => {
    for (const malformed of [null, undefined, 'nope', {}]) {
      expect(parseDocumentsResourceData(malformed)).toEqual(EMPTY);
    }
  });

  it('drops a generation object missing the required booleans', () => {
    const data = parseDocumentsResourceData({ generation: { hasResume: true }, documents: [] });
    expect(data.generation).toBeNull();
  });

  it('drops a malformed document entry (missing name) without failing the whole list', () => {
    const data = parseDocumentsResourceData({
      generation: null,
      documents: [{ id: 'doc-1' }, { id: 'doc-2', name: 'Good.pdf' }],
    });
    expect(data.documents).toEqual([{ id: 'doc-2', name: 'Good.pdf' }]);
  });
});

describe('buildCandidates', () => {
  const URL = 'https://example.com/job/1';

  it('returns empty when there is no generation and no documents', () => {
    expect(buildCandidates(EMPTY, URL)).toEqual([]);
  });

  it('puts the generation candidate first, labelled by title + company', () => {
    const result = buildCandidates(
      {
        generation: {
          hasResume: true,
          hasCoverLetter: true,
          jobTitle: 'Engineer',
          company: 'Acme',
        },
        documents: [{ id: 'doc-1', name: 'Base.pdf' }],
      },
      URL
    );
    expect(result).toEqual([
      { source: { kind: 'generation', url: URL }, label: 'Engineer · Acme', hasCoverLetter: true },
      { source: { kind: 'document', id: 'doc-1' }, label: 'Base.pdf', hasCoverLetter: false },
    ]);
  });

  it('omits the generation candidate when it has no résumé at all', () => {
    const result = buildCandidates(
      { generation: { hasResume: false, hasCoverLetter: false }, documents: [] },
      URL
    );
    expect(result).toEqual([]);
  });

  it('falls back to "This job" when the generation has neither title nor company', () => {
    const result = buildCandidates(
      { generation: { hasResume: true, hasCoverLetter: false }, documents: [] },
      URL
    );
    expect(result[0]?.label).toBe('This job');
  });
});
