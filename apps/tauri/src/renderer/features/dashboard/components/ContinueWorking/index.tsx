import { Bookmark, Briefcase, FileText, Play } from 'lucide-react';
import { useMemo } from 'react';

import type { JobInteraction } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { GlassCard } from '@ajh/ui';

import { useDocuments, useInteractions } from '@/services';

function timeAgo(ts: number) {
  const s = Math.max(1, Math.round((Date.now() - ts) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  if (s < 86400 * 7) return `${Math.round(s / 86400)}d ago`;
  return new Date(ts).toLocaleDateString();
}

export function ContinueWorking() {
  const { t } = useTranslation();

  const { data: docsRaw = [] } = useDocuments();
  const { data: bookmarkedRaw = [] } = useInteractions('bookmarked');
  const { data: appliedRaw = [] } = useInteractions('applied');

  const items = useMemo(() => {
    const docs = docsRaw.slice(0, 3).map((d) => ({
      id: `doc-${d.id}`,
      name: d.title,
      sub: d.source.toUpperCase(),
      ts: d.importedAt,
      icon: FileText,
      color: 'text-blue-400',
    }));

    const bookmarked = (bookmarkedRaw as JobInteraction[]).slice(0, 2).map((i) => ({
      id: `bm-${i.jobId}`,
      name: i.title,
      sub: i.company,
      ts: i.timestamp,
      icon: Bookmark,
      color: 'text-amber-400',
    }));

    const applied = (appliedRaw as JobInteraction[]).slice(0, 2).map((i) => ({
      id: `ap-${i.jobId}`,
      name: i.title,
      sub: i.company,
      ts: i.timestamp,
      icon: Briefcase,
      color: 'text-emerald-400',
    }));

    return [...docs, ...bookmarked, ...applied].sort((a, b) => b.ts - a.ts).slice(0, 4);
  }, [docsRaw, bookmarkedRaw, appliedRaw]);

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          <Play size={14} />
          {t('dashboard.continueWorking')}
        </div>
      </div>

      {items.length === 0 ? (
        <div className="py-6 text-center text-xs text-foreground/30">
          {t('dashboard.noContinueWorking')}
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((item) => {
            const Icon = item.icon;
            return (
              <div
                key={item.id}
                className="group flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5 transition-all hover:bg-white/10"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5">
                  <Icon size={14} className={item.color} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-foreground">{item.name}</div>
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
