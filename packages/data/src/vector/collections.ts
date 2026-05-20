export const COLLECTIONS = ['resumes', 'jobs', 'skills', 'conversations'] as const;
export type CollectionName = (typeof COLLECTIONS)[number];

export interface VectorRecord {
  id: string;
  vector: number[];
  text: string;
  metadata: Record<string, unknown>;
  [key: string]: unknown;
}
