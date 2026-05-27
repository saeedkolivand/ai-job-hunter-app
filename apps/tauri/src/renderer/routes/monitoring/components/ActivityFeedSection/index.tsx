import { AnimatePresence } from 'motion/react';

import { Button, GlassCard } from '@ajh/ui';

import type { ActivityItem } from '../../types';
import { ActivityRow } from '../ActivityRow';

interface Props {
  activity: ActivityItem[];
  onClear: () => void;
  t: (key: string) => string;
}

export function ActivityFeedSection({ activity, onClear, t }: Props) {
  return (
    <GlassCard tone="graphite" highlight className="col-span-3">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/40">
            {t('monitoring.sections.activityFeed')}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {activity.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={onClear}
              className="text-[10px] text-foreground/30 hover:text-foreground/60 h-auto py-1"
            >
              {t('monitoring.actions.clear')}
            </Button>
          )}
          <span className="flex items-center gap-1.5 text-[10px] text-emerald-300/85">
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" />
            {t('monitoring.actions.live')}
          </span>
        </div>
      </div>
      <div className="max-h-72 space-y-1.5 overflow-y-auto pr-1">
        <AnimatePresence initial={false}>
          {activity.length === 0 ? (
            <div className="py-8 text-center text-xs text-foreground/30">
              {t('monitoring.emptyStates.waitingForActivity')}
            </div>
          ) : (
            activity.map((a) => <ActivityRow key={a.id} a={a} />)
          )}
        </AnimatePresence>
      </div>
    </GlassCard>
  );
}
