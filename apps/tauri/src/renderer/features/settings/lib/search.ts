import type { SectionId } from '@/features/settings/constants';
import { SEARCH_INDEX, type SearchEntry } from '@/features/settings/lib/search-index';

export interface SearchResult extends SearchEntry {
  /** Localized section label (e.g. "AI" / "KI") for the result row subtitle. */
  sectionLabel: string;
  /** Localized title resolved from titleKey. */
  title: string;
}

/** Minimal translate signature — satisfied by both `TFunction` and test stubs. */
type Translate = (key: string) => string;

/**
 * Returns entries whose localized title, keywords, or localized section label
 * contain the query as a case-insensitive substring.
 *
 * `sectionLabelKey` maps a SectionId to its i18n key — callers derive this from
 * `NAV_GROUPS` so this function stays pure (no static import of constants).
 */
export function matchEntries(
  query: string,
  t: Translate,
  sectionLabelKeys: Record<SectionId, string>
): SearchResult[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  return SEARCH_INDEX.flatMap((entry) => {
    const title = t(entry.titleKey);
    const sectionLabel = t(sectionLabelKeys[entry.section] ?? '');
    const haystack = [title.toLowerCase(), sectionLabel.toLowerCase(), ...entry.keywords].join(' ');
    return haystack.includes(q) ? [{ ...entry, title, sectionLabel }] : [];
  });
}
