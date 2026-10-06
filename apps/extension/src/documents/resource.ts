/**
 * Pure data shaping for the Documents tab — the `documents` read-tier
 * resource's `data` guard and the picker's candidate list. No DOM, no I/O.
 */

import type { ExtensionDocumentSource } from '@ajh/shared/extension-protocol';

/** One candidate document the picker can export from. */
export interface DocumentCandidate {
  source: ExtensionDocumentSource;
  label: string;
  /** Only a `generation` source can ever have a cover letter. */
  hasCoverLetter: boolean;
}

interface GenerationSummary {
  hasResume: boolean;
  hasCoverLetter: boolean;
  jobTitle?: string;
  company?: string;
}

interface DocumentSummary {
  id: string;
  name: string;
}

interface DocumentsResourceData {
  generation: GenerationSummary | null;
  documents: DocumentSummary[];
}

/** Hand-written guard for the `documents` resource's `data` shape (PR2
 *  §A.2) — the extension stays zod-free everywhere, same discipline as
 *  `bridge.ts`. Ignores fields this picker doesn't render (`targetLanguage`,
 *  `updatedAt`, `language`) rather than validating every one of them. */
export function parseDocumentsResourceData(data: unknown): DocumentsResourceData {
  const EMPTY: DocumentsResourceData = { generation: null, documents: [] };
  if (typeof data !== 'object' || data === null) return EMPTY;
  const o = data as Record<string, unknown>;

  let generation: GenerationSummary | null = null;
  if (typeof o.generation === 'object' && o.generation !== null) {
    const g = o.generation as Record<string, unknown>;
    if (typeof g.hasResume === 'boolean' && typeof g.hasCoverLetter === 'boolean') {
      generation = {
        hasResume: g.hasResume,
        hasCoverLetter: g.hasCoverLetter,
        jobTitle: typeof g.jobTitle === 'string' ? g.jobTitle : undefined,
        company: typeof g.company === 'string' ? g.company : undefined,
      };
    }
  }

  const documents = (Array.isArray(o.documents) ? o.documents : [])
    .filter((d): d is Record<string, unknown> => typeof d === 'object' && d !== null)
    .filter((d) => typeof d.id === 'string' && typeof d.name === 'string')
    .map((d) => ({ id: d.id as string, name: d.name as string }));

  return { generation, documents };
}

/**
 * Build the picker's candidate list from the resource data — the job's own
 * generation first (when it has a résumé to export at all), then the saved
 * base résumés, newest first (the resource itself orders them that way — see
 * the Rust doc). `url` is the active tab's url (background-resolved, echoed
 * back — see `PopupResponse.documentsList`'s doc).
 */
export function buildCandidates(data: DocumentsResourceData, url: string): DocumentCandidate[] {
  const candidates: DocumentCandidate[] = [];
  if (data.generation?.hasResume) {
    const title = data.generation.jobTitle?.trim();
    const company = data.generation.company?.trim();
    const label = title && company ? `${title} · ${company}` : (title ?? company ?? 'This job');
    candidates.push({
      source: { kind: 'generation', url },
      label,
      hasCoverLetter: data.generation.hasCoverLetter,
    });
  }
  for (const doc of data.documents) {
    candidates.push({
      source: { kind: 'document', id: doc.id },
      label: doc.name,
      hasCoverLetter: false,
    });
  }
  return candidates;
}
