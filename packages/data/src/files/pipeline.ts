/**
 * File Processing Pipeline (smart, not naive).
 *
 *  File Import → Type Detection → Text Extraction → OCR Detection →
 *  OCR (if needed) → Chunking → Embeddings → Semantic Indexing → Searchable
 *
 * This module orchestrates the pipeline. Heavy work (OCR, embeddings, chunking)
 * is dispatched to workers / AI runtime; the pipeline itself is I/O glue.
 */
import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { nanoid } from 'nanoid';

import { createLogger } from '@ajh/core';

import { extractDocx } from './docx.js';
import { extractPdf } from './pdf.js';

export interface PipelineDeps {
  ocr(filePath: string, lang?: string): Promise<{ text: string; lang: string }>;
  chunk(text: string): Promise<string[]>;
  embed(texts: string[]): Promise<number[][]>;
  index(
    records: Array<{
      id: string;
      vector: number[];
      text: string;
      metadata: Record<string, unknown>;
    }>
  ): Promise<void>;
  persistDocument(doc: {
    id: string;
    title: string;
    source: string;
    path: string;
    language?: string;
    pages?: number;
  }): Promise<void>;
  persistChunks(
    rows: Array<{ id: string; documentId: string; seq: number; text: string }>
  ): Promise<void>;
}

export interface ImportOptions {
  filePath: string;
  title?: string;
  locale?: string;
  onProgress?: (p: number, stage: string) => void;
}

export class FilePipeline {
  private readonly logger = createLogger('file-pipeline');
  constructor(private readonly deps: PipelineDeps) {}

  async import(opts: ImportOptions): Promise<{ documentId: string; chunks: number }> {
    const ext = path.extname(opts.filePath).toLowerCase().replace('.', '');
    const documentId = nanoid();
    const title = opts.title ?? path.basename(opts.filePath, path.extname(opts.filePath));
    opts.onProgress?.(0.05, 'detect');

    let text: string;
    let pages: number | undefined;
    let source = 'txt';

    if (ext === 'pdf') {
      source = 'pdf';
      opts.onProgress?.(0.15, 'extract-pdf');
      const pdf = await extractPdf(opts.filePath, readFile);
      pages = pdf.pages;
      text = pdf.text;
      if (pdf.needsOcr) {
        this.logger.info({ documentId }, 'pdf needs OCR — running');
        opts.onProgress?.(0.3, 'ocr');
        const ocr = await this.deps.ocr(opts.filePath, opts.locale);
        text = [text, ocr.text].filter(Boolean).join('\n\n');
      }
    } else if (ext === 'docx') {
      source = 'docx';
      opts.onProgress?.(0.2, 'extract-docx');
      text = (await extractDocx(opts.filePath)).text;
    } else if (['png', 'jpg', 'jpeg', 'tiff', 'bmp'].includes(ext)) {
      source = 'image';
      opts.onProgress?.(0.2, 'ocr');
      text = (await this.deps.ocr(opts.filePath, opts.locale)).text;
    } else {
      source = 'txt';
      text = (await readFile(opts.filePath, 'utf8')).toString();
    }

    await this.deps.persistDocument({
      id: documentId,
      title,
      source,
      path: opts.filePath,
      ...(opts.locale ? { language: opts.locale } : {}),
      ...(pages !== undefined ? { pages } : {}),
    });

    opts.onProgress?.(0.55, 'chunk');
    const chunks = await this.deps.chunk(text);

    await this.deps.persistChunks(
      chunks.map((c, i) => ({ id: nanoid(), documentId, seq: i, text: c }))
    );

    opts.onProgress?.(0.7, 'embed');
    const vectors = await this.deps.embed(chunks);

    opts.onProgress?.(0.9, 'index');
    await this.deps.index(
      chunks.map((c, i) => ({
        id: `${documentId}:${i}`,
        vector: vectors[i] ?? [],
        text: c,
        metadata: { documentId, seq: i, title, source },
      }))
    );

    opts.onProgress?.(1, 'done');
    return { documentId, chunks: chunks.length };
  }
}
