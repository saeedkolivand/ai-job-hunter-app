import type { RawDoc } from '@/lib/doc-record';
import { useDocuments } from '@/services';

/** The default saved résumé, with the name a surface can SHOW. */
export interface DefaultResume {
  /** The backend's `_id` — what every command that resolves a résumé takes. */
  id: string;
  /** The document's own file name. Empty when the backend stored none, which
   *  is why callers must have a nameless fallback rather than print `''`. */
  name: string;
}

/**
 * Resolve the default saved résumé, falling back to the first saved one.
 *
 * The NAME matters wherever a surface generates from a résumé the user is not
 * looking at (the jobs pane runs the staged pipeline against this document, not
 * against anything on screen) — a surface that silently picks one is the
 * failure the label exists to prevent.
 */
export function useDefaultResume(): DefaultResume | null {
  const { data = [] } = useDocuments();
  const docs = data as unknown as RawDoc[];
  const def = docs.find((d) => d.isDefault) ?? docs[0];
  if (!def?._id) return null;
  return { id: def._id, name: def.name?.trim() ?? '' };
}

/** Resolve the default saved résumé's id (`_id`), falling back to the first saved one. */
export function useDefaultResumeId(): string | null {
  return useDefaultResume()?.id ?? null;
}
