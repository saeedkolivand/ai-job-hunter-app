import { describe, expect, it } from 'vitest';

import {
  ExtensionDocumentExportRequestSchema,
  ExtensionDocumentExportResultSchema,
  ExtensionEnvelopeSchema,
} from '../extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from '../extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionDocumentExportRequestSchema / ExtensionDocumentExportResultSchema (PR2)
// ---------------------------------------------------------------------------

describe('ExtensionDocumentExportRequestSchema', () => {
  it('accepts a résumé export from a generation source', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'generation', url: 'https://example.com/job/123' },
        kind: 'resume',
        format: 'pdf',
        templateId: 'classic',
      })
    ).not.toThrow();
  });

  it('accepts a cover-letter export from a document source, with letterLayoutId', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'document', id: 'doc-1' },
        kind: 'cover-letter',
        format: 'txt',
        templateId: 'classic',
        letterLayoutId: 'banded',
        atsMode: true,
      })
    ).not.toThrow();
  });

  it('rejects an unknown source kind', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'upload', url: 'https://example.com/job/123' },
        kind: 'resume',
        format: 'pdf',
        templateId: 'classic',
      })
    ).toThrow();
  });

  it('rejects an unknown kind', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'document', id: 'doc-1' },
        kind: 'transcript',
        format: 'pdf',
        templateId: 'classic',
      })
    ).toThrow();
  });

  it('rejects an unknown format', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'document', id: 'doc-1' },
        kind: 'resume',
        format: 'rtf',
        templateId: 'classic',
      })
    ).toThrow();
  });

  it('rejects a request with no templateId', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'document', id: 'doc-1' },
        kind: 'resume',
        format: 'pdf',
      })
    ).toThrow();
  });

  it('rejects a generation source with an empty url', () => {
    expect(() =>
      ExtensionDocumentExportRequestSchema.parse({
        source: { kind: 'generation', url: '' },
        kind: 'resume',
        format: 'pdf',
        templateId: 'classic',
      })
    ).toThrow();
  });
});

describe('ExtensionDocumentExportResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = {
      ok: true,
      data: 'JVBERi0xLjQK',
      dataEncoding: 'base64',
      mimeType: 'application/pdf',
      filename: 'resume.pdf',
      byteLength: 9,
      kind: 'resume',
      format: 'pdf',
      templateId: 'classic',
    };
    expect(ExtensionDocumentExportResultSchema.parse(payload)).toEqual(payload);
  });

  it('rejects a success payload whose dataEncoding is not the literal base64', () => {
    expect(() =>
      ExtensionDocumentExportResultSchema.parse({
        ok: true,
        data: 'JVBERi0xLjQK',
        dataEncoding: 'utf8',
        mimeType: 'application/pdf',
        filename: 'resume.pdf',
        byteLength: 9,
        kind: 'resume',
        format: 'pdf',
        templateId: 'classic',
      })
    ).toThrow();
  });

  it('accepts a user-facing refusal payload', () => {
    expect(() =>
      ExtensionDocumentExportResultSchema.parse({
        ok: false,
        error: 'Assisted autofill is off.',
      })
    ).not.toThrow();
  });

  it('round-trips a throttle refusal carrying detail + retryAfterMs', () => {
    const payload = {
      ok: false,
      error: 'rate_limited',
      detail: 'Too many requests — try again shortly.',
      retryAfterMs: 500,
    };
    expect(ExtensionDocumentExportResultSchema.parse(payload)).toEqual(payload);
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionDocumentExportResultSchema.parse({})).toThrow();
  });

  it('rejects an incomplete ok:true payload missing filename', () => {
    expect(() =>
      ExtensionDocumentExportResultSchema.parse({
        ok: true,
        data: 'JVBERi0xLjQK',
        dataEncoding: 'base64',
        mimeType: 'application/pdf',
        byteLength: 9,
        kind: 'resume',
        format: 'pdf',
        templateId: 'classic',
      })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionDocumentExportResultSchema.parse({ ok: false, filename: 'resume.pdf' })
    ).toThrow();
  });

  it('carries document.export / document.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.documentExport,
        reqId: 'req-021',
        payload: {
          source: { kind: 'generation', url: 'https://example.com/job/123' },
          kind: 'resume',
          format: 'pdf',
          templateId: 'classic',
        },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.documentResult,
        reqId: 'req-022',
        payload: {
          ok: true,
          data: 'JVBERi0xLjQK',
          dataEncoding: 'base64',
          mimeType: 'application/pdf',
          filename: 'resume.pdf',
          byteLength: 9,
          kind: 'resume',
          format: 'pdf',
          templateId: 'classic',
        },
      })
    ).not.toThrow();
  });
});
