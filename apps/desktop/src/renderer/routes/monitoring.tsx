import { Activity, BarChart3, CheckCircle2, Clock, Cpu, Loader2, XCircle, Zap } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button, GlassCard } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { fetchJob, useAppVersion, useJobEvents, useJobQueue, useSystemHealth } from '@/services';

export const Route = createFileRoute('/monitoring')({ component: MonitoringPage });

interface JobEvent {
  type:
    | 'job.queued'
    | 'job.started'
    | 'job.progress'
    | 'job.stream'
    | 'job.completed'
    | 'job.failed'
    | 'job.cancelled';
  jobId: string;
  data?: unknown;
  ts: number;
}

interface JobRecord {
  id: string;
  kind: string;
  status: 'queued' | 'running' | 'streaming' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  createdAt: number;
  updatedAt: number;
}

interface ActivityItem {
  id: string;
  time: number;
  text: string;
  tone: 'violet' | 'indigo' | 'blue' | 'emerald' | 'amber';
}

const KIND_LABEL: Record<string, string> = {
  'ai.generate': 'AI generation',
  'ai.embed': 'Embedding',
  'document.import': 'Document import',
  'document.ocr': 'OCR run',
  'document.chunk': 'Chunking',
  'document.index': 'Indexing',
  'scrape.board': 'Board scrape',
  'scrape.url': 'URL scrape',
  'persist.job': 'Interaction saved',
  'match.resume': 'Resume match',
  'apply.job': 'Application',
};

const KIND_SHORT: Record<string, string> = {
  'ai.generate': 'AI',
  'ai.embed': 'Embed',
  'document.import': 'Doc',
  'scrape.board': 'Scrape',
  'scrape.url': 'Scrape',
  'persist.job': 'Save',
  'match.resume': 'Match',
  'apply.job': 'Apply',
};

// Persist session counters across navigation via module-level state
let _counters = { completed: 0, running: 0, failed: 0, items: 0 };
let _last24h: number[] = Array.from({ length: 24 }, () => 0);
let _activity: ActivityItem[] = [];

function MonitoringPage() {
  const { t } = useTranslation();

  const KIND_LABEL: Record<string, string> = {
    'ai.generate': t('monitoring.jobKinds.aiGenerate'),
    'ai.embed': t('monitoring.jobKinds.aiEmbed'),
    'document.import': t('monitoring.jobKinds.documentImport'),
    'document.ocr': t('monitoring.jobKinds.documentOcr'),
    'document.chunk': t('monitoring.jobKinds.documentChunk'),
    'document.index': t('monitoring.jobKinds.documentIndex'),
    'scrape.board': t('monitoring.jobKinds.scrapeBoard'),
    'scrape.url': t('monitoring.jobKinds.scrapeUrl'),
    'persist.job': t('monitoring.jobKinds.persistJob'),
    'match.resume': t('monitoring.jobKinds.matchResume'),
    'apply.job': t('monitoring.jobKinds.applyJob'),
  };

  const [activity, setActivity] = useState<ActivityItem[]>(_activity);
  const [counters, setCounters] = useState(_counters);
  const [last24h, setLast24h] = useState<number[]>(_last24h);

  const { data: healthData } = useSystemHealth();
  const health = (healthData ?? {}) as {
    ai?: { ready: boolean; model?: string };
    data?: { ready: boolean; sqlite: boolean; vector: boolean };
  };
  const { data: allJobs } = useJobQueue();
  const activeJobs = ((allJobs ?? []) as JobRecord[]).filter(
    (j) => j.status === 'queued' || j.status === 'running' || j.status === 'streaming'
  );
  const { data: appVersionData } = useAppVersion();
  const appVersion = (appVersionData as string | undefined) ?? '';

  // Sync module-level cache when state updates
  useEffect(() => {
    _counters = counters;
  }, [counters]);
  useEffect(() => {
    _last24h = last24h;
  }, [last24h]);
  useEffect(() => {
    _activity = activity;
  }, [activity]);

  // Subscribe to job events
  useJobEvents((ev: unknown) => {
    const event = ev as JobEvent;
    void (async () => {
      const job = (await fetchJob(event.jobId)) as JobRecord | null;
      const kindLabel = (job?.kind && KIND_LABEL[job.kind]) ?? 'Job';
      const tone: ActivityItem['tone'] =
        event.type === 'job.completed'
          ? 'emerald'
          : event.type === 'job.failed'
            ? 'amber'
            : event.type === 'job.cancelled'
              ? 'amber'
              : job?.kind?.startsWith('scrape')
                ? 'violet'
                : job?.kind?.startsWith('ai')
                  ? 'indigo'
                  : 'blue';

      if (['job.completed', 'job.failed', 'job.cancelled', 'job.started'].includes(event.type)) {
        const verb =
          event.type === 'job.completed'
            ? '✓'
            : event.type === 'job.failed'
              ? '✕'
              : event.type === 'job.cancelled'
                ? '⊘'
                : '▸';
        const item: ActivityItem = {
          id: `${event.jobId}-${event.ts}`,
          time: event.ts,
          text: `${verb} ${kindLabel}`,
          tone,
        };
        setActivity((prev) => {
          const next = [item, ...prev].slice(0, 40);
          _activity = next;
          return next;
        });
      }

      setCounters((c) => {
        let next = c;
        if (event.type === 'job.completed')
          next = { ...c, completed: c.completed + 1, running: Math.max(0, c.running - 1) };
        else if (event.type === 'job.failed' || event.type === 'job.cancelled')
          next = { ...c, failed: c.failed + 1, running: Math.max(0, c.running - 1) };
        else if (event.type === 'job.started') next = { ...c, running: c.running + 1 };
        else if (event.type === 'job.stream') next = { ...c, items: c.items + 1 };
        _counters = next;
        return next;
      });

      if (event.type === 'job.completed' || event.type === 'job.stream') {
        const hour = new Date(event.ts).getHours();
        setLast24h((bins) => {
          const next = bins.map((v, i) => (i === hour ? v + 1 : v));
          _last24h = next;
          return next;
        });
      }
    })();
  });

  const total = counters.completed + counters.failed;
  const successRate = total ? Math.round((counters.completed / total) * 100) : 100;

  const metrics = [
    {
      label: t('monitoring.metrics.completed'),
      value: counters.completed,
      icon: CheckCircle2,
      color: 'text-emerald-400',
      bg: 'bg-emerald-500/10',
    },
    {
      label: t('monitoring.metrics.running'),
      value: counters.running,
      icon: Loader2,
      color: 'text-blue-400',
      bg: 'bg-blue-500/10',
    },
    {
      label: t('monitoring.metrics.failed'),
      value: counters.failed,
      icon: XCircle,
      color: 'text-amber-400',
      bg: 'bg-amber-500/10',
    },
    {
      label: t('monitoring.metrics.successRate'),
      value: `${successRate}%`,
      icon: Zap,
      color: 'text-purple-400',
      bg: 'bg-purple-500/10',
    },
  ];

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-5xl space-y-5">
        <PageHeader
          title={t('monitoring.title')}
          subtitle={t('monitoring.subtitle')}
          badge={t('monitoring.badge')}
          actions={
            <div className="flex items-center gap-4 text-[11px]">
              <StatusDot
                label={t('monitoring.status.ollama')}
                ready={health.ai?.ready ?? false}
                detail={health.ai?.model}
              />
              <StatusDot
                label={t('monitoring.status.database')}
                ready={health.data?.ready ?? false}
              />
              {appVersion && <span className="text-foreground/30">v{appVersion}</span>}
            </div>
          }
        />

        {/* Metric cards */}
        <div className="grid grid-cols-4 gap-3">
          {metrics.map(({ label, value, icon: Icon, color, bg }) => (
            <div
              key={label}
              className="relative overflow-hidden rounded-xl border border-white/[0.07] px-4 py-4"
              style={{
                background:
                  'linear-gradient(135deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01))',
              }}
            >
              <div className={cn('mb-3 flex h-8 w-8 items-center justify-center rounded-lg', bg)}>
                <Icon
                  size={15}
                  className={cn(
                    color,
                    label === 'Running' && counters.running > 0 ? 'animate-spin' : ''
                  )}
                />
              </div>
              <div className="text-2xl font-semibold tabular-nums text-foreground/90">{value}</div>
              <div className="mt-0.5 text-[11px] text-foreground/40">{label}</div>
            </div>
          ))}
        </div>

        {/* Active jobs + Activity feed */}
        <div className="grid grid-cols-5 gap-4">
          {/* Active jobs */}
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
                    <ActiveJobRow key={job.id} job={job} kindLabel={KIND_LABEL} t={t} />
                  ))}
                </AnimatePresence>
              )}
            </div>
          </GlassCard>

          {/* Activity feed */}
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
                    onClick={() => {
                      setActivity([]);
                      _activity = [];
                    }}
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
        </div>

        {/* Sparkline */}
        <GlassCard tone="graphite" highlight>
          <div className="mb-3 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <BarChart3 size={12} className="text-brand-soft" />
              <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/40">
                {t('monitoring.sections.hourlyActivity')}
              </span>
            </div>
            <span className="text-[10px] text-foreground/30">
              {t('monitoring.timeLabels.last24Hours')}
            </span>
          </div>
          <Sparkline data={last24h} />
        </GlassCard>

        {/* Footer */}
        <div className="flex items-center justify-between rounded-xl border border-white/[0.05] px-4 py-3 text-[11px] text-foreground/35">
          <div className="flex items-center gap-2">
            <Cpu size={11} className="text-foreground/25" />
            {t('monitoring.footer.localProcessing')}
          </div>
          <span>{t('monitoring.footer.localPrivateOffline')}</span>
        </div>
      </div>
    </PageTransition>
  );
}

function StatusDot({ label, ready, detail }: { label: string; ready: boolean; detail?: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span
        className={cn('h-1.5 w-1.5 rounded-full', ready ? 'bg-emerald-400' : 'bg-amber-400/70')}
      />
      <span className={cn(ready ? 'text-foreground/55' : 'text-foreground/35')}>
        {label}
        {detail ? ` · ${detail}` : ''}
      </span>
    </span>
  );
}

function ActiveJobRow({
  job,
  t,
}: {
  job: JobRecord;
  kindLabel: Record<string, string>;
  t: (key: string) => string;
}) {
  const _label = KIND_SHORT[job.kind] ?? job.kind;
  const isStreaming = job.status === 'streaming';

  return (
    <motion.div
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 4 }}
      transition={transition.normal}
      className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2"
    >
      <div className="flex items-center justify-between gap-2 mb-1.5">
        <div className="flex items-center gap-1.5">
          <Loader2 size={10} className="animate-spin text-brand-soft" />
          <span className="text-xs font-medium text-foreground/80">
            {KIND_LABEL[job.kind] ?? job.kind}
          </span>
        </div>
        <span
          className={cn(
            'text-[10px] font-medium',
            isStreaming ? 'text-blue-400' : 'text-foreground/40'
          )}
        >
          {isStreaming ? t('monitoring.timeLabels.streaming') : job.status}
        </span>
      </div>
      {job.progress > 0 && (
        <div className="h-0.5 w-full overflow-hidden rounded-full bg-white/[0.06]">
          <div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary transition-all duration-300"
            style={{ width: `${Math.round(job.progress * 100)}%` }}
          />
        </div>
      )}
      <div className="mt-1 text-[10px] text-foreground/30 font-mono">{job.id.slice(0, 12)}…</div>
    </motion.div>
  );
}

function ActivityRow({ a }: { a: ActivityItem }) {
  const ago = useAgo(a.time);
  const dotClass = {
    violet: 'bg-violet-400',
    indigo: 'bg-indigo-400',
    blue: 'bg-blue-400',
    emerald: 'bg-emerald-400',
    amber: 'bg-amber-400',
  }[a.tone];

  return (
    <motion.div
      initial={{ opacity: 0, x: 6 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0 }}
      transition={transition.fast}
      className="flex items-start gap-2.5 rounded-lg px-2 py-1.5 hover:bg-white/[0.02] transition-colors"
    >
      <span
        className={cn(
          'mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full shadow-[0_0_6px_currentColor]',
          dotClass
        )}
      />
      <div className="flex-1 text-[12px] text-foreground/70">{a.text}</div>
      <div className="flex shrink-0 items-center gap-0.5 text-[10px] text-foreground/30">
        <Clock size={9} />
        {ago}
      </div>
    </motion.div>
  );
}

function Sparkline({ data }: { data: number[] }) {
  const max = Math.max(1, ...data);
  const now = new Date().getHours();

  return (
    <div className="relative">
      <svg viewBox="0 0 480 64" preserveAspectRatio="none" className="h-20 w-full">
        <defs>
          <linearGradient id="bar-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="rgba(168,85,247,0.6)" />
            <stop offset="100%" stopColor="rgba(168,85,247,0.1)" />
          </linearGradient>
          <linearGradient id="bar-fill-now" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="rgba(99,102,241,0.9)" />
            <stop offset="100%" stopColor="rgba(99,102,241,0.2)" />
          </linearGradient>
        </defs>
        {data.map((v, i) => {
          const slotW = 480 / data.length;
          const x = i * slotW + slotW * 0.2;
          const w = slotW * 0.6;
          const h = Math.max(2, (v / max) * 56);
          return (
            <rect
              key={i}
              x={x}
              y={64 - h}
              width={w}
              height={h}
              rx={2}
              fill={i === now ? 'url(#bar-fill-now)' : 'url(#bar-fill)'}
            />
          );
        })}
      </svg>
      {/* Hour labels */}
      <div className="mt-1 flex justify-between text-[9px] text-foreground/25 px-0.5">
        {['12am', '3am', '6am', '9am', '12pm', '3pm', '6pm', '9pm'].map((l) => (
          <span key={l}>{l}</span>
        ))}
      </div>
    </div>
  );
}

function useAgo(ts: number): string {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((n) => n + 1), 15_000);
    return () => clearInterval(id);
  }, []);
  const s = Math.max(1, Math.round((Date.now() - ts) / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.round(s / 60)}m`;
  return `${Math.round(s / 3600)}h`;
}
