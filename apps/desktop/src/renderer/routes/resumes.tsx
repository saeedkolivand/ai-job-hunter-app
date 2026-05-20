import { useState, useMemo } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { motion, AnimatePresence } from 'motion/react';
import { useTranslation } from '@/lib/i18n';
import {
  Send,
  Eye,
  Bookmark,
  ExternalLink,
  Building2,
  MapPin,
  Clock,
  Search,
  RefreshCw,
} from 'lucide-react';
import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { Button } from '@/components/ui/Button';
import { CardSkeleton } from '@/components/ui/LoadingSkeleton';
import { EmptyState } from '@/components/ui/EmptyState';
import { useInteractions } from '@/services/use-postings';
import { useOpenExternal } from '@/services/use-system';
import { stagger, transition } from '@/lib/motion';
import { cn } from '@/lib/cn';

export const Route = createFileRoute('/resumes')({ component: Resumes });

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

type Tab = 'applied' | 'viewed' | 'bookmarked';

const TAB_CONFIG = [
  {
    id: 'applied' as Tab,
    labelKey: 'resumes.tabs.applied',
    icon: Send,
    color: 'text-purple-300',
    ringColor: 'border-purple-400/30 bg-purple-400/10',
  },
  {
    id: 'viewed' as Tab,
    labelKey: 'resumes.tabs.viewed',
    icon: Eye,
    color: 'text-blue-300',
    ringColor: 'border-blue-400/30 bg-blue-400/10',
  },
  {
    id: 'bookmarked' as Tab,
    labelKey: 'resumes.tabs.bookmarked',
    icon: Bookmark,
    color: 'text-amber-300',
    ringColor: 'border-amber-400/30 bg-amber-400/10',
  },
] as const;

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

function Resumes() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>('applied');
  const [filter, setFilter] = useState('');

  const { data: rows = [], isLoading, refetch } = useInteractions(tab);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q
      ? (rows as Interaction[]).filter(
          (r) => r.title.toLowerCase().includes(q) || r.company.toLowerCase().includes(q)
        )
      : (rows as Interaction[]);
  }, [rows, filter]);

  const tabCfg = TAB_CONFIG.find((c) => c.id === tab) as (typeof TAB_CONFIG)[number];

  return (
    <PageTransition className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-10 py-10">
        <PageHeader
          title={t('resumes.title')}
          subtitle={t('resumes.subtitle')}
          badge={t('resumes.badge')}
          actions={
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-1.5 transition-colors focus-within:border-brand/35">
                <Search size={12} className="shrink-0 text-foreground/40" />
                <input
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder={t('resumes.filterPlaceholder')}
                  className="w-40 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25"
                />
              </div>
              <Button size="sm" variant="ghost" onClick={() => void refetch()} title="Refresh">
                <RefreshCw size={12} className={isLoading ? 'animate-spin' : ''} />
              </Button>
            </div>
          }
        />

        {/* Tabs */}
        <div className="mb-5 flex items-center gap-1">
          {TAB_CONFIG.map(({ id, labelKey, icon: Icon, color }) => (
            <button
              key={id}
              onClick={() => {
                setTab(id);
                setFilter('');
              }}
              className={cn(
                'flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-150',
                tab === id
                  ? 'bg-white/[0.07] text-foreground/90 ring-1 ring-white/10'
                  : 'text-foreground/45 hover:bg-white/[0.04] hover:text-foreground/70'
              )}
            >
              <Icon size={12} className={tab === id ? color : ''} />
              {t(labelKey)}
              {tab === id && rows.length > 0 && (
                <span className="rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] text-foreground/60">
                  {rows.length}
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Content */}
        {isLoading ? (
          <div className="space-y-2">
            <CardSkeleton /> <CardSkeleton /> <CardSkeleton />
          </div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={tabCfg.icon}
            title={
              filter ? t('resumes.noResults') : t('resumes.noJobsYet', { tab: t(tabCfg.labelKey) })
            }
            description={!filter ? t('resumes.jobsWillAppear') : undefined}
          />
        ) : (
          <motion.div
            className="flex flex-col gap-2"
            variants={stagger.container}
            initial="hidden"
            animate="show"
          >
            <AnimatePresence initial={false}>
              {filtered.map((row) => (
                <motion.div
                  key={`${row.jobId}-${row.interactionType}`}
                  variants={stagger.item}
                  transition={transition.normal}
                  exit={{ opacity: 0, y: -6 }}
                >
                  <InteractionRow row={row} tabCfg={tabCfg} />
                </motion.div>
              ))}
            </AnimatePresence>
          </motion.div>
        )}
      </div>
    </PageTransition>
  );
}

function InteractionRow({
  row,
  tabCfg,
}: {
  row: Interaction;
  tabCfg: (typeof TAB_CONFIG)[number];
}) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const Icon = tabCfg.icon;

  return (
    <div className="glass-graphite glass-highlight flex items-center gap-4 rounded-xl p-4 transition-colors hover:bg-white/[0.02]">
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
        <button
          onClick={() => void openExternal.mutate(row.url)}
          className="flex shrink-0 items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground"
        >
          <ExternalLink size={11} /> {t('resumes.open')}
        </button>
      )}
    </div>
  );
}
