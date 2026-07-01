import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

const ocrFile = vi.fn();
vi.mock('@/lib/ocr', () => ({ ocrFile: (...a: unknown[]) => ocrFile(...a) }));

import { useImportWithOcr } from './use-import-with-ocr';

// jsdom's File does not implement arrayBuffer(); a minimal stub is enough since
// the hook only reads `.name` and `.arrayBuffer()` and forwards the file to the
// (mocked) OCR routine.
function makeFile(): File {
  return {
    name: 'cv.pdf',
    arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer,
  } as unknown as File;
}

afterEach(() => vi.clearAllMocks());

describe('useImportWithOcr', () => {
  it('returns the import result directly for a normal document', async () => {
    const importDoc = vi.fn().mockResolvedValue({ id: 'doc-1' });
    const client = createMockClient({ 'documents.import': importDoc });
    const { result } = renderHook(() => useImportWithOcr(), { wrapper: withProviders(client) });

    let out: unknown;
    await act(async () => {
      out = await result.current.importFile(makeFile());
    });
    expect(out).toEqual({ id: 'doc-1' });
    expect(ocrFile).not.toHaveBeenCalled();
    await waitFor(() => expect(result.current.status).toBe('idle'));
  });

  it('falls back to OCR and re-imports for a scanned PDF', async () => {
    const importDoc = vi
      .fn()
      .mockResolvedValueOnce({ error: 'scanned_pdf' })
      .mockResolvedValueOnce({ id: 'doc-ocr' });
    ocrFile.mockResolvedValue({ text: 'recovered text', confidence: 80 });
    const client = createMockClient({ 'documents.import': importDoc });
    const { result } = renderHook(() => useImportWithOcr(), { wrapper: withProviders(client) });

    let out: unknown;
    await act(async () => {
      out = await result.current.importFile(makeFile());
    });
    expect(ocrFile).toHaveBeenCalledOnce();
    expect(out).toEqual({ id: 'doc-ocr' });
    // The second import is the OCR text as a .txt file.
    expect(importDoc).toHaveBeenCalledTimes(2);
    expect(importDoc.mock.calls[1]?.[0]).toMatchObject({ name: 'cv.txt', title: 'cv.pdf' });
  });

  it('throws when OCR yields no text', async () => {
    const importDoc = vi.fn().mockResolvedValue({ error: 'scanned_pdf' });
    ocrFile.mockResolvedValue({ text: '   ', confidence: 0 });
    const client = createMockClient({ 'documents.import': importDoc });
    const { result } = renderHook(() => useImportWithOcr(), { wrapper: withProviders(client) });

    await expect(result.current.importFile(makeFile())).rejects.toThrow(/no text/);
  });
});
