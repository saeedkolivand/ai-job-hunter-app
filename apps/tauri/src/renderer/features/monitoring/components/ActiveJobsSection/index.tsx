import { Activity } from 'lucide-react';
import { AnimatePresence } from 'motion/react';

import { GlassCard } from '@ajh/ui';

import { ActiveJobRow } from '@/features/monitoring/components/ActiveJobRow';
import type { JobRecord } from '@/features/monitoring/types';

interface Props {
  activeJobs: JobRecord[];
  kindLabel: Record<string, string>;
  t: (key: string) => string;
}

export function ActiveJobsSection({ activeJobs, kindLabel, t }: Props) {
  return (
    <GlassCard tone="graphite" highlight className="col-span-2">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Activity size={12} className="text-brand-soft" />
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/40">
            Active Jobs
          </span>
        </div>
        {activeJobs.length > 0 && (
          <span className="flex h-4 w-4 items-center justify-center rounded-full bg-brand/20 text-[9px] font-bold text-brand-soft">
            {activeJobs.length}
          </span>
        )}
      </div>

      <div className="space-y-2 min-h-[120px]">
        {activeJobs.length === 0 ? (
          <div className="flex h-24 items-center justify-center text-xs text-foreground/25">
            {t('monitoring.emptyStates.noActiveJobs')}
          </div>
        ) : (
          <AnimatePresence initial={false}>
            {activeJobs.map((job) => (
              <ActiveJobRow key={job.id} job={job} kindLabel={kindLabel} t={t} />
            ))}
          </AnimatePresence>
        )}
      </div>
    </GlassCard>
  );
}
