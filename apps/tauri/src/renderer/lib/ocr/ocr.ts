/** Map i18n locale (e.g. "de", "de-AT") to a Tesseract language code. */
function tesseractLang(locale: string): string {
  const base = locale.split('-')[0]?.toLowerCase() ?? 'en';
  const map: Record<string, string> = {
    de: 'deu',
    en: 'eng',
    fr: 'fra',
    es: 'spa',
    nl: 'nld',
    pl: 'pol',
    it: 'ita',
    pt: 'por',
  };
  return map[base] ?? 'eng';
}

export interface OcrResult {
  text: string;
  confidence: number;
}

/**
 * Run Tesseract.js OCR on a File or Blob.
 * Loaded lazily — the WASM bundle is only fetched when actually needed.
 */
export async function ocrFile(file: File | Blob, locale: string): Promise<OcrResult> {
  const { createWorker } = await import('tesseract.js');
  const lang = tesseractLang(locale);
  const worker = await createWorker(lang);
  try {
    const { data } = await worker.recognize(file);
    return { text: data.text, confidence: data.confidence };
  } finally {
    await worker.terminate();
  }
}
