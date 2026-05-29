import { afterEach, describe, expect, it, vi } from 'vitest';

const recognize = vi.fn();
const terminate = vi.fn();
const createWorker = vi.fn(async (_lang?: string) => ({ recognize, terminate }));

vi.mock('tesseract.js', () => ({ createWorker: (lang?: string) => createWorker(lang) }));

import { ocrFile } from './ocr';

afterEach(() => {
  vi.clearAllMocks();
});

describe('ocrFile', () => {
  it('recognises text and returns text + confidence', async () => {
    recognize.mockResolvedValue({ data: { text: 'hello', confidence: 92 } });
    const result = await ocrFile(new Blob(['x']), 'en');
    expect(result).toEqual({ text: 'hello', confidence: 92 });
    expect(terminate).toHaveBeenCalledOnce();
  });

  it('maps the locale to the matching Tesseract language code', async () => {
    recognize.mockResolvedValue({ data: { text: '', confidence: 0 } });
    await ocrFile(new Blob(['x']), 'de-AT');
    expect(createWorker).toHaveBeenCalledWith('deu');
  });

  it('falls back to English for unknown locales', async () => {
    recognize.mockResolvedValue({ data: { text: '', confidence: 0 } });
    await ocrFile(new Blob(['x']), 'xx');
    expect(createWorker).toHaveBeenCalledWith('eng');
  });

  it('terminates the worker even when recognition fails', async () => {
    recognize.mockRejectedValue(new Error('boom'));
    await expect(ocrFile(new Blob(['x']), 'en')).rejects.toThrow('boom');
    expect(terminate).toHaveBeenCalledOnce();
  });
});
