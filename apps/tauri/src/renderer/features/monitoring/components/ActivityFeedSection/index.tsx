import { Inbox } from 'lucide-react';
import { AnimatePresence } from 'motion/react';

import { EmptyState, GlassCard } from '@ajh/ui';

import { ActivityRow } from '@/features/monitoring/components/ActivityRow';
import type { ActivityItem } from '@/features/monitoring/types';

interface Props {
  activity: ActivityItem[];
  t: (key: string) => string;
}

export function ActivityFeedSection({ activity, t }: Props) {
  return (
    <GlassCard tone="graphite" highlight className="col-span-3">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('monitoring.sections.activityFeed')}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="flex items-center gap-1.5 text-[10px] text-emerald-300/85">
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" />
            {t('monitoring.actions.live')}
          </span>
        </div>
      </div>
      <div className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
        <AnimatePresence initial={false}>
          {activity.length === 0 ? (
            <EmptyState
              icon={Inbox}
              title={t('monitoring.emptyStates.waitingForActivity')}
              className="py-8"
            />
          ) : (
            activity.map((a) => <ActivityRow key={a.id} a={a} />)
          )}
        </AnimatePresence>
      </div>
    </GlassCard>
  );
}
