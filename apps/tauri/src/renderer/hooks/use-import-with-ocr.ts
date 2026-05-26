import { useState } from 'react';

import i18n from 'i18next';

import { ocrFile } from '@/lib/ocr';
import { useImportDocument } from '@/services';

export type ImportWithOcrStatus = 'idle' | 'importing' | 'ocr' | 'retrying';

/**
 * Wraps useImportDocument with a Tesseract.js fallback for scanned PDFs.
 *
 * Flow: import → if backend returns scanned_pdf → OCR the file → re-import
 * the OCR text as a .txt document under the same title.
 */
export function useImportWithOcr() {
  const importDocument = useImportDocument();
  const [status, setStatus] = useState<ImportWithOcrStatus>('idle');

  const importFile = async (file: File) => {
    setStatus('importing');
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const result = (await importDocument.mutateAsync({
        name: file.name,
        bytes,
        title: file.name,
      })) as { id?: string; success?: boolean; error?: string };

      if (result?.error !== 'scanned_pdf') {
        return result;
      }

      // Scanned PDF — fall back to Tesseract.js OCR.
      setStatus('ocr');
      const { text } = await ocrFile(file, i18n.language);

      if (!text.trim()) {
        throw new Error('OCR returned no text. The document may be too low-resolution.');
      }

      setStatus('retrying');
      const ocrBytes = new TextEncoder().encode(text);
      const baseName = file.name.replace(/\.[^.]+$/, '');
      return await importDocument.mutateAsync({
        name: `${baseName}.txt`,
        bytes: new Uint8Array(ocrBytes),
        title: file.name,
      });
    } finally {
      setStatus('idle');
    }
  };

  return {
    importFile,
    status,
    isPending: status !== 'idle',
    isOcr: status === 'ocr',
  };
}
