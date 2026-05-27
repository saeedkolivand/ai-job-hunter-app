import { Activity, BarChart3, CheckCircle2, Clock, Cpu, Loader2, XCircle, Zap } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button, cn, GlassCard, transition } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { useTranslation } from '@/lib/i18n';
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
  finishedAt?: number;
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
  'autopilot.run': 'Autopilot run',
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
  'autopilot.run': 'Autopilot',
};

function MonitoringPage() {
  const { t } = useTranslation();

  const KIND_LABEL_MAP = useMemo(
    () =>
      ({
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
        'autopilot.run': t('monitoring.jobKinds.autopilotRun'),
      }) as Record<string, string>,
    [t]
  );

  // Live-only events not yet reflected in the refetched queue
  const [liveActivity, setLiveActivity] = useState<ActivityItem[]>([]);

  const { data: healthData } = useSystemHealth();
  const health = (healthData ?? {}) as {
    ai?: { ready: boolean; model?: string };
    data?: { ready: boolean; sqlite: boolean; vector: boolean };
  };
  const { data: allJobsData } = useJobQueue();
  const { data: appVersionData } = useAppVersion();
  const appVersion = (appVersionData as string | undefined) ?? '';

  const allJobs = useMemo(() => (allJobsData ?? []) as JobRecord[], [allJobsData]);

  // Counters derived from real job queue data
  const activeJobs = useMemo(
    () =>
      allJobs.filter(
        (j) => j.status === 'queued' || j.status === 'running' || j.status === 'streaming'
      ),
    [allJobs]
  );
  const completedCount = useMemo(
    () => allJobs.filter((j) => j.status === 'completed').length,
    [allJobs]
  );
  const failedCount = useMemo(
    () => allJobs.filter((j) => j.status === 'failed' || j.status === 'cancelled').length,
    [allJobs]
  );

  // Hourly activity chart derived from finishedAt timestamps
  const last24h = useMemo(() => {
    const bins = Array.from({ length: 24 }, () => 0);
    allJobs
      .filter((j) => j.status === 'completed')
      .forEach((j) => {
        const ts = j.finishedAt ?? j.updatedAt;
        const h = new Date(ts).getHours();
        if (h >= 0 && h < 24) bins[h] = (bins[h] ?? 0) + 1;
      });
    return bins;
  }, [allJobs]);

  // Historical activity from completed/failed jobs
  const historicalActivity = useMemo(() => {
    return allJobs
      .filter((j) => j.status === 'completed' || j.status === 'failed' || j.status === 'cancelled')
      .sort((a, b) => (b.finishedAt ?? b.updatedAt) - (a.finishedAt ?? a.updatedAt))
      .slice(0, 40)
      .map((j) => {
        const verb = j.status === 'completed' ? '✓' : j.status === 'failed' ? '✕' : '⊘';
        const tone: ActivityItem['tone'] =
          j.status !== 'completed'
            ? 'amber'
            : j.kind?.startsWith('scrape')
              ? 'violet'
              : j.kind?.startsWith('ai')
                ? 'indigo'
                : 'emerald';
        return {
          id: j.id,
          time: j.finishedAt ?? j.updatedAt,
          text: `${verb} ${KIND_LABEL_MAP[j.kind] ?? j.kind}`,
          tone,
        };
      });
  }, [allJobs, KIND_LABEL_MAP]);

  // Merge live (top) with historical (deduped)
  const activity = useMemo(() => {
    const liveJobIds = new Set(liveActivity.map((a) => a.id.split('-')[0]));
    return [...liveActivity, ...historicalActivity.filter((a) => !liveJobIds.has(a.id))].slice(
      0,
      40
    );
  }, [liveActivity, historicalActivity]);

  // Subscribe to job events — prepend fresh events to live list
  useJobEvents((ev: unknown) => {
    const event = ev as JobEvent;
    void (async () => {
      const job = (await fetchJob(event.jobId)) as JobRecord | null;
      const kindLabel = (job?.kind && KIND_LABEL_MAP[job.kind]) ?? 'Job';
      const tone: ActivityItem['tone'] =
        event.type === 'job.completed'
          ? job?.kind?.startsWith('scrape')
            ? 'violet'
            : job?.kind?.startsWith('ai')
              ? 'indigo'
              : 'emerald'
          : event.type === 'job.failed' || event.type === 'job.cancelled'
            ? 'amber'
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
        setLiveActivity((prev) =>
          [
            {
              id: `${event.jobId}-${event.ts}`,
              time: event.ts,
              text: `${verb} ${kindLabel}`,
              tone,
            },
            ...prev,
          ].slice(0, 40)
        );
      }
    })();
  });

  const total = completedCount + failedCount;
  const successRate = total ? Math.round((completedCount / total) * 100) : 100;
  const counters = { completed: completedCount, running: activeJobs.length, failed: failedCount };

  const KIND_LABEL = KIND_LABEL_MAP;

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
                    onClick={() => setLiveActivity([])}
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
