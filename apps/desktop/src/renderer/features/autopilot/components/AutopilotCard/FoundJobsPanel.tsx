import { ChevronUp } from 'lucide-react';
import { motion } from 'motion/react';

import type { AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, transition } from '@ajh/ui';

import type { FoundJobsSortBy } from './found-jobs';
import { FoundJobRow } from './FoundJobRow';

interface Props {
  jobs: AutopilotFoundJob[];
  sortBy: FoundJobsSortBy;
  onSortChange: (value: FoundJobsSortBy) => void;
  onCollapse: () => void;
  /** Fired when the expand animation completes (the focus-scroll hook). */
  onAnimationComplete: () => void;
  listContainerRef: React.RefObject<HTMLDivElement | null>;
  highlightedUrl: string | null;
  mixedScoreSources: boolean;
  viewedUrls: Set<string>;
  splitPending: boolean;
  onOpen: (job: AutopilotFoundJob) => void;
  onApply: (job: AutopilotFoundJob) => void;
  onSplitCluster: (job: AutopilotFoundJob) => void;
}

/** Found jobs from the most recent run — header with the per-card sort, then the rows. */
export function FoundJobsPanel({
  jobs,
  sortBy,
  onSortChange,
  onCollapse,
  onAnimationComplete,
  listContainerRef,
  highlightedUrl,
  mixedScoreSources,
  viewedUrls,
  splitPending,
  onOpen,
  onApply,
  onSplitCluster,
}: Props) {
  const { t } = useTranslation();
  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: 'auto' }}
      exit={{ opacity: 0, height: 0 }}
      transition={transition.fast}
      className="overflow-hidden"
      onAnimationComplete={onAnimationComplete}
    >
      <div className="overflow-hidden rounded-lg border border-[var(--border-clear)] bg-card">
        <div className="flex items-center justify-between border-b border-[var(--border-clear)] px-3 py-2">
          <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('autopilot.foundJobs')} · {jobs.length}
          </span>
          {/* Per-card view-side sort (local state in AutopilotCard): each card
              sorts its own found-jobs list independently. */}
          <div className="flex items-center gap-1">
            <Dropdown
              options={[
                { value: 'relevance', label: t('autopilot.sortRelevance') },
                { value: 'newest', label: t('jobs.sortNewest') },
                { value: 'oldest', label: t('jobs.sortOldest') },
              ]}
              value={sortBy}
              onChange={(value) => onSortChange(value as FoundJobsSortBy)}
              size="sm"
              placeholder={t('jobs.sort')}
              aria-label={t('jobs.sort')}
            />
            <Button
              variant="unstyled"
              type="button"
              onClick={onCollapse}
              aria-label={t('autopilot.collapse')}
              title={t('autopilot.collapse')}
              className="rounded p-1 text-foreground/30 transition-colors hover:text-foreground/70"
            >
              <ChevronUp size={14} />
            </Button>
          </div>
        </div>
        <div
          ref={listContainerRef}
          className="max-h-64 divide-y divide-[var(--border-clear)] overflow-y-auto"
        >
          {jobs.map((job, i) => (
            <FoundJobRow
              key={`${job.url}-${i}`}
              job={job}
              highlighted={highlightedUrl === job.url}
              mixedScoreSources={mixedScoreSources}
              viewed={viewedUrls.has(job.url)}
              splitPending={splitPending}
              onOpen={onOpen}
              onApply={onApply}
              onSplitCluster={onSplitCluster}
            />
          ))}
        </div>
      </div>
    </motion.div>
  );
}
