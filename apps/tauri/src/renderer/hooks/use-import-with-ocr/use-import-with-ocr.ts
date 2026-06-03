import { useState } from 'react';

import type { ContactFieldConflict, ContactProfile, StructuredResume } from '@ajh/shared';

import i18n from '@/i18n';
import { ocrFile } from '@/lib/ocr';
import { useImportDocument } from '@/services';

export type ImportWithOcrStatus = 'idle' | 'importing' | 'ocr' | 'retrying';

type ImportResult = {
  id?: string;
  success?: boolean;
  error?: string;
  review?: StructuredResume;
  contactConflicts?: ContactFieldConflict[];
  suggestedContact?: ContactProfile;
};

/**
 * Wraps useImportDocument with a Tesseract.js fallback for scanned PDFs.
 *
 * Flow: import → if backend returns scanned_pdf → OCR the file → re-import
 * the OCR text as a .txt document under the same title.
 */
export function useImportWithOcr() {
  const importDocument = useImportDocument();
  const [status, setStatus] = useState<ImportWithOcrStatus>('idle');
  // Structured-extraction review from the most recent import (per-field
  // confidence + missing-field warnings); surfaced when `reviewRequired`.
  const [review, setReview] = useState<StructuredResume | null>(null);

  const importFile = async (file: File): Promise<ImportResult> => {
    setStatus('importing');
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const result = (await importDocument.mutateAsync({
        name: file.name,
        bytes,
        title: file.name,
      })) as ImportResult;

      if (result?.error !== 'scanned_pdf') {
        setReview(result?.review ?? null);
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
      const retried = (await importDocument.mutateAsync({
        name: `${baseName}.txt`,
        bytes: new Uint8Array(ocrBytes),
        title: file.name,
      })) as ImportResult;
      setReview(retried?.review ?? null);
      return retried;
    } finally {
      setStatus('idle');
    }
  };

  return {
    importFile,
    status,
    isPending: status !== 'idle',
    isOcr: status === 'ocr',
    review,
    clearReview: () => setReview(null),
  };
}
