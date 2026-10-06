import type { Posting } from '@/features/jobs/types';

/**
 * A minimal valid Posting. Distinct `id` gives a distinct url, so
 * `mergePostings`' canonical-key dedup keeps every row unless a test overrides it.
 */
export function makePosting(id: string, overrides: Partial<Posting> = {}): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: id,
    company: 'Acme',
    description: '',
    capturedAt: 0,
    ...overrides,
  };
}
