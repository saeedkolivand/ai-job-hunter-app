import { Bookmark, Briefcase, CheckCircle, Eye, TrendingUp } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { GlassCard } from '@ajh/ui';

import { useInteractions } from '@/services';

/**
 * Interaction types that count toward "tracked" in the pipeline overview.
 * An explicit allowlist — not "every type except `dismissed`" — so a future
 * SIXTH interaction type must be deliberately added here before it can
 * silently inflate this headline number. `dismissed` is excluded on purpose:
 * a job the user explicitly rejected was never "tracked".
 */
const TRACKED_INTERACTION_TYPES = new Set(['viewed', 'opened', 'applied', 'bookmarked']);

export function JobPipelineOverview() {
  const { t } = useTranslation();

  const { data: bookmarked = [] } = useInteractions('bookmarked');
  const { data: applied = [] } = useInteractions('applied');
  const { data: viewed = [] } = useInteractions('viewed');
  const { data: allInteractions = [] } = useInteractions();

  // `useInteractions()` (no filter) returns every interaction regardless of
  // type — including `dismissed`, which is not tracking, it's the opposite.
  const trackedInteractions = (allInteractions as { interactionType?: string }[]).filter((i) =>
    TRACKED_INTERACTION_TYPES.has(i.interactionType ?? '')
  );

  const stats = [
    {
      label: t('dashboard.savedJobs'),
      value: (bookmarked as unknown[]).length,
      icon: Bookmark,
      color: 'text-blue-400',
      bg: 'bg-blue-400/10',
    },
    {
      label: t('dashboard.applied'),
      value: (applied as unknown[]).length,
      icon: CheckCircle,
      color: 'text-emerald-400',
      bg: 'bg-emerald-400/10',
    },
    {
      label: t('dashboard.viewed'),
      value: (viewed as unknown[]).length,
      icon: Eye,
      color: 'text-orange-400',
      bg: 'bg-orange-400/10',
    },
    {
      label: t('dashboard.totalTracked'),
      value: trackedInteractions.length,
      icon: TrendingUp,
      color: 'text-purple-400',
      bg: 'bg-purple-400/10',
    },
  ];

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        <Briefcase size={14} />
        {t('dashboard.jobPipeline')}
      </div>

      <div className="grid grid-cols-1 gap-3 @xs:grid-cols-2">
        {stats.map((stat) => {
          const Icon = stat.icon;
          return (
            <div
              key={stat.label}
              className="flex flex-col items-center gap-2 rounded-xl border border-foreground/[0.06] bg-foreground/[0.03] px-3 py-3.5"
            >
              <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${stat.bg}`}>
                <Icon size={15} className={stat.color} />
              </div>
              <div className="text-3xl font-bold tabular-nums text-foreground">{stat.value}</div>
              <div className="text-center text-[11px] text-muted-foreground">{stat.label}</div>
            </div>
          );
        })}
      </div>

      {trackedInteractions.length === 0 && (
        <p className="mt-3 text-center text-xs text-muted-foreground">
          {t('dashboard.noJobsTracked')}
        </p>
      )}
    </GlassCard>
  );
}
