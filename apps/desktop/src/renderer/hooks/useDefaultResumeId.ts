import type { RawDoc } from '@/lib/doc-record';
import { useDocuments } from '@/services';

/** Resolve the default saved résumé's id (`_id`), falling back to the first saved one. */
export function useDefaultResumeId(): string | null {
  const { data = [] } = useDocuments();
  const docs = data as unknown as RawDoc[];
  const def = docs.find((d) => d.isDefault) ?? docs[0];
  return def?._id ?? null;
}
