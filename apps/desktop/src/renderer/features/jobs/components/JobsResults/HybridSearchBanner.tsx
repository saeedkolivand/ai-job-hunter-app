import type { HybridSearchArms } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import type { PostingsSearchState } from '@/features/jobs/hooks/usePostingsSearch';

/** Hybrid-search UI state `JobsPage` hands down — see `usePostingsSearch` for
 *  where each field comes from. Optional on `JobsResults`: callers that never
 *  wire a search (and every EXISTING test) get the `idle` default, so
 *  `filtered` is treated as the plain substring-filtered list. */
export interface HybridSearchUi {
  /** Gated to the currently-typed filter text — see `JobsPage`. */
  state: PostingsSearchState;
  arms: HybridSearchArms | null;
  /** How many postings the search actually ranked over (eligible subset). */
  corpusSize: number;
  onRetry: () => void;
  onClear: () => void;
  onEnableSemanticRanking: () => void;
}

/** A `·`-separated degraded-arm notice. */
function ArmNote({ children }: { children: React.ReactNode }) {
  return (
    <>
      <span aria-hidden="true" className="text-foreground/30">
        ·
      </span>
      {children}
    </>
  );
}

/**
 * "Ranked by …" banner above a search's own results — surfaces which arms
 * actually ran (never lets a keyword-only list present as hybrid) and, when
 * semantic ranking is off, a one-click enable action instead of just a note.
 */
export function HybridSearchBanner({ hybridSearch }: { hybridSearch: HybridSearchUi }) {
  const { t } = useTranslation();
  const { arms } = hybridSearch;
  return (
    <div
      data-testid={TEST_IDS.jobs.searchBanner}
      role="status"
      className="mb-2 flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border border-foreground/10 bg-foreground/[0.03] px-3 py-2 text-[11px] text-foreground/60"
    >
      <span>
        {t('jobs.hybridSearch.rankedBy', {
          count: hybridSearch.corpusSize,
          arms: [
            arms?.lexical === 'ran' ? t('jobs.hybridSearch.armLexical') : null,
            arms?.dense === 'ran' ? t('jobs.hybridSearch.armDense') : null,
            arms?.rerank === 'ran' ? t('jobs.hybridSearch.armRerank') : null,
          ]
            .filter((label): label is string => label !== null)
            .join(', '),
        })}
      </span>
      {arms?.dense === 'skipped' && (
        <ArmNote>
          <span>{t('jobs.hybridSearch.semanticOff')}</span>
          <Button variant="ghost" onClick={hybridSearch.onEnableSemanticRanking}>
            {t('jobs.hybridSearch.enableSemanticRanking')}
          </Button>
        </ArmNote>
      )}
      {arms?.dense === 'unavailable' && (
        <ArmNote>
          <span>{t('jobs.hybridSearch.semanticUnavailable')}</span>
        </ArmNote>
      )}
      {arms?.rerank === 'unavailable' && (
        <ArmNote>
          <span>{t('jobs.hybridSearch.rerankUnavailable')}</span>
        </ArmNote>
      )}
      <Button variant="ghost" className="ml-auto" onClick={hybridSearch.onClear}>
        {t('jobs.hybridSearch.clearSearch')}
      </Button>
    </div>
  );
}
