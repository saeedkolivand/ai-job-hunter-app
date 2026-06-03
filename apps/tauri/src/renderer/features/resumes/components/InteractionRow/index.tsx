import { Building2, Clock, ExternalLink, MapPin } from 'lucide-react';

import { Button, cn, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useOpenExternal } from '@/services/use-system';

interface Interaction {
  jobId: string;
  interactionType: string;
  timestamp: number;
  title: string;
  company: string;
  url: string;
  source: string;
  location: string;
}

interface TabConfig {
  id: string;
  labelKey: string;
  icon: React.ElementType;
  color: string;
  ringColor: string;
}

interface InteractionRowProps {
  row: Interaction;
  tabCfg: TabConfig;
}

function formatRelative(ts: number, t: ReturnType<typeof useTranslation>['t']): string {
  const diff = Date.now() - ts;
  const m = Math.floor(diff / 60000);
  const h = Math.floor(m / 60);
  const d = Math.floor(h / 24);
  if (m < 1) return t('resumes.relativeTime.justNow');
  if (m < 60) return t('resumes.relativeTime.minutesAgo', { m });
  if (h < 24) return t('resumes.relativeTime.hoursAgo', { h });
  if (d < 7) return t('resumes.relativeTime.daysAgo', { d });
  return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function InteractionRow({ row, tabCfg }: InteractionRowProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const Icon = tabCfg.icon;

  return (
    <GlassCard
      tone="graphite"
      className="flex items-center gap-4 rounded-xl p-4 transition-colors hover:bg-white/[0.02]"
    >
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/5 text-[10px] uppercase tracking-wider text-brand-soft">
        {row.source.slice(0, 2)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-medium text-foreground/90">
          <span className="truncate">{row.title || t('resumes.unknownPosition')}</span>
          <span
            className={cn(
              'flex shrink-0 items-center gap-1 rounded-full border px-1.5 py-0.5 text-[9px] uppercase tracking-wider',
              tabCfg.ringColor,
              tabCfg.color
            )}
          >
            <Icon size={8} /> {t(tabCfg.labelKey)}
          </span>
        </div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-foreground/50">
          {row.company && (
            <span className="flex items-center gap-1">
              <Building2 size={10} /> {row.company}
            </span>
          )}
          {row.location && (
            <span className="flex items-center gap-1">
              <MapPin size={10} /> {row.location}
            </span>
          )}
          <span className="flex items-center gap-1 text-foreground/35">
            <Clock size={10} /> {formatRelative(row.timestamp, t)}
          </span>
          <span className="text-foreground/30">{row.source}</span>
        </div>
      </div>

      {row.url && (
        <Button
          onClick={() => void openExternal.mutate(row.url)}
          className="flex shrink-0 items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
        >
          <ExternalLink size={11} /> {t('resumes.open')}
        </Button>
      )}
    </GlassCard>
  );
}
