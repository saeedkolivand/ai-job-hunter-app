/**
 * Wire-format helpers for DocumentRecord.
 *
 * The Rust backend serialises `id` as `_id` and `created_at` as `createdAt`.
 * These three exports are the single canonical source for the shape + the
 * normalise transform; import from here instead of re-declaring locally.
 */

import type { DocumentRecord } from '@ajh/shared';

/** Raw shape returned by the Rust backend before normalisation. */
export type RawDoc = Omit<DocumentRecord, 'id' | 'importedAt'> & {
  _id: string;
  createdAt: number;
  name?: string;
  text?: string; // returned by the backend but not in the shared TS type
};

/** Normalise a raw backend document to the shared `DocumentRecord` shape. */
export function normalise(raw: RawDoc): DocumentRecord {
  return {
    ...raw,
    id: raw._id,
    importedAt: raw.createdAt,
    source:
      raw.source ??
      (raw.name?.endsWith('.pdf') ? 'pdf' : raw.name?.endsWith('.docx') ? 'docx' : 'txt'),
  };
}

/** Type guard: true when `x` carries the raw `_id` field from the backend. */
export function isRawDoc(x: unknown): x is RawDoc {
  return typeof x === 'object' && x !== null && typeof (x as { _id?: unknown })._id === 'string';
}
