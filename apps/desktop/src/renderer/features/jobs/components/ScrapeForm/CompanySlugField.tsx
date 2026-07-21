import { type Ref, useEffect, useState } from 'react';

import type { BoardCatalogEntry, DiscoveredCompany } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  type CompanyOption,
  CompanyTypeahead,
  type CompanyTypeaheadHandle,
  SetupHint,
  useNotification,
} from '@ajh/ui';

import { useCompanySearch, useSetStarred } from '@/services/use-discovery';

interface CompanySlugFieldProps {
  /** Forwarded to the typeahead so the scrape submit path can flush a pending slug. */
  ref?: Ref<CompanyTypeaheadHandle>;
  /** Current slug list submitted to the scrape (the form's `companies`). */
  companies: string[];
  onChange: (companies: string[]) => void;
  /** The selected listed boards — their curated seeds merge into suggestions. */
  seededBoards: readonly BoardCatalogEntry[];
  disabled?: boolean;
}

/** Debounce a value so the typeahead fires one search per pause, not per keystroke. */
function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return debounced;
}

/**
 * Merge server-discovered rows with the selected boards' curated seeds into one
 * suggestion list (ADR-030 §d). Discovered rows win on a `(atsKind, slug)`
 * collision (they carry real seen-count/starred state); curated-only seeds are
 * marked so the UI can flag them subtly. `query` filters the curated seeds
 * client-side (the discovered rows are already filtered server-side).
 *
 * Keyed by a lowercased `atsKind:slug` STRING in a Map — never a dynamic object
 * key on scraped data (Feature-B security advisory).
 */
export function mergeCompanyOptions(
  discovered: readonly DiscoveredCompany[],
  seededBoards: readonly BoardCatalogEntry[],
  query: string
): CompanyOption[] {
  const byKey = new Map<string, CompanyOption>();
  for (const d of discovered) {
    // Key is lowercased for collision detection, but the ORIGINAL slug casing is
    // preserved in the value — Ashby company tokens are case-sensitive.
    byKey.set(`${d.atsKind.toLowerCase()}:${d.slug.toLowerCase()}`, {
      atsKind: d.atsKind,
      slug: d.slug,
      displayName: d.displayName,
      seenCount: d.seenCount,
      starred: d.starred,
      curated: false,
    });
  }

  const q = query.trim().toLowerCase();
  for (const board of seededBoards) {
    for (const name of board.seededCompanies ?? []) {
      if (q && !name.toLowerCase().includes(q)) continue;
      const key = `${board.id.toLowerCase()}:${name.toLowerCase()}`;
      if (byKey.has(key)) continue; // a discovered row already covers it
      byKey.set(key, {
        atsKind: board.id,
        slug: name,
        displayName: name,
        seenCount: 0,
        starred: false,
        curated: true,
      });
    }
  }

  return [...byKey.values()];
}

/**
 * ATS slug typeahead (ADR-030 §d) — replaces the old comma-separated company
 * input. Typing searches the passively-harvested slug store (merged with the
 * selected boards' curated seeds); slugs are added as removable chips that feed
 * the scrape `companies` array, and each suggestion carries a star (watch)
 * toggle. Free-text is always addable so an unknown slug is never a dead end.
 */
export function CompanySlugField({
  ref,
  companies,
  onChange,
  seededBoards,
  disabled,
}: CompanySlugFieldProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const [query, setQuery] = useState('');
  const debouncedQuery = useDebounced(query, 250);

  const { data: discovered, isFetching } = useCompanySearch({ query: debouncedQuery });
  const setStarred = useSetStarred();

  const suggestions = mergeCompanyOptions(discovered ?? [], seededBoards, debouncedQuery).filter(
    // A slug already selected doesn't need to appear as an addable suggestion.
    (s) => !companies.includes(s.slug)
  );

  const addCompany = (slug: string) => {
    if (companies.includes(slug)) return;
    onChange([...companies, slug]);
  };
  const removeCompany = (slug: string) => {
    onChange(companies.filter((c) => c !== slug));
  };

  const toggleStar = (option: CompanyOption) => {
    setStarred.mutate(
      { atsKind: option.atsKind, slug: option.slug, starred: !option.starred },
      { onError: () => notify.error({ message: t('jobs.discovery.starFailed') }) }
    );
  };

  return (
    <CompanyTypeahead
      ref={ref}
      id="scrape-companies"
      selected={companies}
      onAdd={addCompany}
      onRemove={removeCompany}
      query={query}
      onQueryChange={setQuery}
      suggestions={suggestions}
      onToggleStar={toggleStar}
      loading={isFetching}
      disabled={disabled}
      placeholder={t('jobs.companies.placeholder')}
      starLabel={(o) => t('jobs.discovery.starToggle', { company: o.displayName || o.slug })}
      removeLabel={(slug) => t('jobs.companies.remove', { company: slug })}
      curatedLabel={t('jobs.discovery.curated')}
      resultsLabel={(n) =>
        n === 0 ? t('jobs.discovery.noMatches') : t('jobs.discovery.resultsCount', { count: n })
      }
      emptyState={<SetupHint message={t('jobs.discovery.slugHint')} />}
      inputTestId={TEST_IDS.jobs.companyTypeahead}
      suggestionTestId={TEST_IDS.jobs.companySuggestion}
      starTestId={TEST_IDS.jobs.companyStarToggle}
      chipTestId={TEST_IDS.jobs.companyChip}
    />
  );
}
