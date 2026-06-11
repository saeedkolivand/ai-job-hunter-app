import { Building2, Clock, ExternalLink, MapPin, Tag } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, GlassCard } from '@ajh/ui';

import { type Interaction, INTERACTION_TYPES } from '@/features/resumes/constants';
import { useOpenExternal } from '@/services/use-system';

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

interface InteractionRowProps {
  row: Interaction;
}

export function InteractionRow({ row }: InteractionRowProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();

  // Self-describing: the Activity feed mixes applied / viewed / bookmarked in one
  // list, so each row's badge reflects its own interaction type. Unknown types
  // (forward-compat) fall back to a neutral badge showing the raw type string.
  const cfg = INTERACTION_TYPES[row.interactionType];
  const Icon = cfg?.icon ?? Tag;
  const label = cfg ? t(cfg.labelKey) : row.interactionType;

  return (
    <GlassCard className="flex items-center gap-4 rounded-xl p-4 transition-colors hover:bg-white/[0.02]">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/5 text-[10px] uppercase tracking-wider text-brand-soft">
        {row.source.slice(0, 2)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-medium text-foreground/90">
          <span className="truncate">{row.title || t('resumes.unknownPosition')}</span>
          <span
            className={cn(
              'flex shrink-0 items-center gap-1 rounded-full border px-1.5 py-0.5 text-[9px] uppercase tracking-wider',
              cfg?.ringColor ?? 'border-white/10 bg-white/5',
              cfg?.color ?? 'text-foreground/50'
            )}
          >
            <Icon size={8} /> {label}
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
