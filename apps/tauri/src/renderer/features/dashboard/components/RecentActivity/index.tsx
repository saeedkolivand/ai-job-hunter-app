import { Bookmark, Briefcase, Clock, ExternalLink, FileText, type LucideIcon } from 'lucide-react';
import { useMemo } from 'react';

import type { DocumentRecord, JobInteraction } from '@ajh/shared';
import { GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useDocuments, useInteractions } from '@/services';

interface ActivityItem {
  key: string;
  title: string;
  sub: string;
  ts: number;
  icon: LucideIcon;
  color: string;
}

function timeAgo(ts: number) {
  const s = Math.max(1, Math.round((Date.now() - ts) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  if (s < 86400 * 7) return `${Math.round(s / 86400)}d ago`;
  return new Date(ts).toLocaleDateString();
}

export function RecentActivity() {
  const { t } = useTranslation();

  const { data: docsRaw = [] } = useDocuments();
  const { data: interactionsRaw = [] } = useInteractions();

  const items = useMemo<ActivityItem[]>(() => {
    const docs = (docsRaw as DocumentRecord[]).map((d) => ({
      key: `doc-${d.id}`,
      title: d.title,
      sub: d.source.toUpperCase(),
      ts: d.importedAt,
      icon: FileText as LucideIcon,
      color: 'text-blue-400',
    }));

    const interactions = (interactionsRaw as JobInteraction[]).map((i) => ({
      key: `int-${i.jobId}-${i.interactionType}`,
      title: i.title,
      sub: i.company,
      ts: i.timestamp,
      icon: (i.interactionType === 'applied'
        ? ExternalLink
        : i.interactionType === 'bookmarked'
          ? Bookmark
          : Briefcase) as LucideIcon,
      color:
        i.interactionType === 'applied'
          ? 'text-emerald-400'
          : i.interactionType === 'bookmarked'
            ? 'text-amber-400'
            : 'text-foreground/40',
    }));

    return [...docs, ...interactions].sort((a, b) => b.ts - a.ts).slice(0, 6);
  }, [docsRaw, interactionsRaw]);

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          <Clock size={14} />
          {t('dashboard.recentActivity')}
        </div>
      </div>

      {items.length === 0 ? (
        <div className="py-6 text-center text-xs text-foreground/30">
          {t('dashboard.noRecentActivity')}
        </div>
      ) : (
        <div className="space-y-3">
          {items.map((item) => {
            const Icon = item.icon;
            return (
              <div
                key={item.key}
                className="flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5 transition-colors hover:bg-white/10"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5">
                  <Icon size={14} className={item.color} />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="truncate text-sm text-foreground">{item.title}</div>
                  <div className="text-xs text-foreground/40">{item.sub}</div>
                </div>
                <span className="shrink-0 text-[11px] text-foreground/30">{timeAgo(item.ts)}</span>
              </div>
            );
          })}
        </div>
      )}
    </GlassCard>
  );
}
