/**
 * PDF text extraction via pdf-parse.
 * Returns extracted text and a heuristic about whether OCR is needed.
 */
import { parsePdf } from './pdf-adapter.js';

export interface PdfExtraction {
  text: string;
  pages: number;
  needsOcr: boolean;
  perPageLengths: number[];
}

const MIN_CHARS_PER_PAGE = 80;

export async function extractPdfFromBytes(bytes: Uint8Array): Promise<PdfExtraction> {
  const buffer = Buffer.from(bytes);
  const data = await parsePdf(buffer);

  const pages = data.numpages;
  const text = data.text.trim();

  // Estimate per-page lengths (pdf-parse doesn't provide this directly)
  const avgLength = text.length / Math.max(1, pages);
  const perPageLengths = Array(pages).fill(avgLength);

  const avg = avgLength;
  return {
    text,
    pages,
    needsOcr: avg < MIN_CHARS_PER_PAGE,
    perPageLengths,
  };
}

export async function extractPdf(
  filePath: string,
  readFile: (p: string) => Promise<Buffer>
): Promise<PdfExtraction> {
  return extractPdfFromBytes(new Uint8Array(await readFile(filePath)));
}
