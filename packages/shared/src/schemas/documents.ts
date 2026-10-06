import { z } from 'zod';

import { LocaleSchema } from './ai.js';

export const DocumentImportRequestSchema = z.object({
  /** Original filename — used to derive title and detect format. */
  name: z.string().min(1).max(512),
  /** Raw file bytes — works in browser (FileReader), Electron, and Tauri alike. */
  bytes: z
    .instanceof(Uint8Array)
    .refine((b) => b.byteLength > 0 && b.byteLength <= 50 * 1024 * 1024, {
      message: 'document must be between 1 byte and 50 MB',
    }),
  title: z.string().optional(),
  locale: LocaleSchema.optional(),
});

export const ResumeExtractTextSchema = z.object({
  name: z.string().min(1).max(512),
  bytes: z
    .instanceof(Uint8Array)
    .refine((b) => b.byteLength > 0 && b.byteLength <= 25 * 1024 * 1024, {
      message: 'file must be between 1 byte and 25 MB',
    }),
});
export type ResumeExtractTextRequest = z.infer<typeof ResumeExtractTextSchema>;

export type DocumentImportRequest = z.infer<typeof DocumentImportRequestSchema>;
