import { useMemo } from 'react';

import type { WorkTypeOption } from '@ajh/shared';

import { matchesWorkTypeFilter } from '@/features/jobs/lib/work-type-filter';
import type { Posting } from '@/features/jobs/types';

interface VisibleFilters {
  filter: string;
  sortBy: 'newest' | 'oldest' | 'company';
  hideAgency: boolean;
  workTypes: WorkTypeOption[];
}

/**
 * The text-filtered, cluster-collapsed, sorted list the Jobs page shows when no
 * hybrid search governs the view, plus the derived counts/allowlists around it.
 */
export function useVisiblePostings(
  allPostings: Posting[],
  { filter, sortBy, hideAgency, workTypes }: VisibleFilters
) {
  // Eligible-id allowlist for hybrid search: the SAME cluster-canonical /
  // hideAgency / workTypes composition `filtered` applies below, MINUS the
  // text-search step — a committed search REPLACES the substring filter
  // rather than compounding with it (ranking a query against postings the
  // substring box already excluded on the SAME text would defeat semantic
  // retrieval's whole point: surfacing matches that don't literally contain
  // the query). Keep these three predicates in lockstep with `filtered`'s.
  const eligibleForSearch = useMemo(() => {
    let result = allPostings.filter((p) => p.clusterCanonical !== false);
    if (hideAgency) result = result.filter((p) => !p.isAgency);
    return result.filter((p) => matchesWorkTypeFilter(p, workTypes));
  }, [allPostings, hideAgency, workTypes]);

  const { filtered, hasDeclaredWorkType } = useMemo(() => {
    let result = allPostings;
    const q = filter.trim().toLowerCase();
    if (q) {
      result = result.filter(
        (p) =>
          p.title.toLowerCase().includes(q) ||
          p.company.toLowerCase().includes(q) ||
          (p.location ?? '').toLowerCase().includes(q)
      );
    }

    // Cross-board clustering (ADR-029): show one row per cluster — the canonical
    // member. Non-canonical members (clusterCanonical === false) collapse into
    // it; unannotated rows (no clusterId → clusterCanonical undefined) ALWAYS
    // show — live-streamed rows are unclustered until the completion refetch.
    result = result.filter((p) => p.clusterCanonical !== false);

    // Optional agency filter — hide recruiting/staffing-agency postings.
    if (hideAgency) result = result.filter((p) => !p.isAgency);

    // Whether the JobsCommandBar work-type control is even worth showing:
    // computed on this EXACT array — the one the work-type filter is about to
    // run over, right below — so the gate and the filter can never disagree.
    // Deliberately measured BEFORE the work-type filter itself: with an active
    // selection the filter narrows this array, and "does the still-visible set
    // declare a type" would trivially say yes for a matching selection and no
    // for one that filters to zero — neither answers the question the control
    // needs ("is there anything for this control to do on THIS search").
    const hasDeclaredWorkType = result.some((p) => p.workType != null);

    // Optional work-type filter — view-only, no re-scrape. An undeclared
    // `workType` is always kept (see `matchesWorkTypeFilter`).
    result = result.filter((p) => matchesWorkTypeFilter(p, workTypes));

    // Stable, deterministic ordering (audit quick win 8): an `id` tiebreak so
    // equal timestamps never reorder between renders (nondeterministic order
    // reads as flakiness), and — for the date sorts — undated postings (no
    // `postedAt`) collect in a trailing band instead of interleaving with
    // genuinely-dated ones via the `capturedAt` fallback (a scrape-time clock,
    // not a posting date, so a just-captured undated row would otherwise jump
    // above a real week-old posting).
    // Ordinal (not localeCompare) — ids are opaque keys, not display text;
    // collation can canonically-equate distinct sequences.
    const byId = (x: Posting, y: Posting) => (x.id < y.id ? -1 : x.id > y.id ? 1 : 0);
    result = [...result].sort((a, b) => {
      if (sortBy === 'company') {
        return a.company.localeCompare(b.company) || byId(a, b);
      }
      // newest / oldest: dated band first, undated (postedAt-less) band last.
      const aDated = typeof a.postedAt === 'number';
      const bDated = typeof b.postedAt === 'number';
      if (aDated !== bDated) return aDated ? -1 : 1;
      const aTime = a.postedAt ?? a.capturedAt;
      const bTime = b.postedAt ?? b.capturedAt;
      const cmp = sortBy === 'oldest' ? aTime - bTime : bTime - aTime;
      return cmp || byId(a, b);
    });

    return { filtered: result, hasDeclaredWorkType };
  }, [allPostings, filter, sortBy, hideAgency, workTypes]);

  // Denominator for the "N / M" count = distinct jobs (clusters): rows that
  // aren't a collapsed non-canonical duplicate. The numerator (`filtered`)
  // reduces this by the text filter + hideAgency, so both count against
  // distinct jobs, not raw postings.
  const distinctCount = useMemo(
    () => allPostings.filter((p) => p.clusterCanonical !== false).length,
    [allPostings]
  );

  return { eligibleForSearch, filtered, hasDeclaredWorkType, distinctCount };
}
