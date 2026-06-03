import {
  Briefcase,
  Check,
  ChevronUp,
  ExternalLink,
  Pause,
  Pencil,
  Play,
  RotateCcw,
  Trash2,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';
import { Button, cn, ConfirmModal, GlassCard, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { type AutopilotRunState, RUN_STATE_LABEL } from '@/lib/machines/autopilot-run.machine';
import { useOpenExternal } from '@/services';

import { ApplyJobModal } from '../ApplyJobModal';

interface StepLog {
  step: string;
  detail: string;
  ts: number;
}

interface AutopilotCardProps {
  autopilot: Autopilot;
  runState: AutopilotRunState;
  stepLogs: StepLog[];
  onRun(): void;
  onTogglePause(): void;
  onEdit(): void;
  onDelete(): void;
}

const STEP_ICON: Record<string, string> = {
  scrape_start: '⟳',
  scrape_done: '✓',
  rank_done: '★',
  cancelled: '⊘',
  complete: '✓',
};

export function AutopilotCard({
  autopilot: ap,
  runState,
  stepLogs,
  onRun,
  onTogglePause,
  onEdit,
  onDelete,
}: AutopilotCardProps) {
  const paused = ap.status === 'paused';
  const running = runState === 'scraping' || runState === 'ranking';
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const [showFound, setShowFound] = useState(false);
  const [applyJob, setApplyJob] = useState<AutopilotFoundJob | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const foundJobs = ap.foundJobs ?? [];

  const lastRun = ap.lastRunAt
    ? new Date(ap.lastRunAt).toLocaleString()
    : t('autopilot.wizard.never');

  return (
    <GlassCard className="flex flex-col gap-3">
      <div className="flex items-center gap-4">
        {/* Status dot */}
        <div
          className={cn(
            'h-2 w-2 rounded-full shrink-0',
            paused
              ? 'bg-foreground/20'
              : running
                ? 'bg-amber-400 animate-pulse'
                : runState === 'error'
                  ? 'bg-red-400'
                  : 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]'
          )}
        />

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-0.5">
            <span className="text-sm font-semibold text-foreground/85 truncate">{ap.name}</span>
            <span className="text-[10px] text-foreground/30 font-mono bg-white/[0.04] px-1.5 py-0.5 rounded">
              {ap.target.board}
            </span>
            <span className="text-[10px] text-foreground/30 bg-white/[0.04] px-1.5 py-0.5 rounded capitalize">
              {ap.schedule.replace('_', ' ')}
            </span>
          </div>
          <div className="flex items-center gap-4 text-[10px] text-foreground/35">
            <span>"{ap.target.query}"</span>
            {ap.target.location && <span>· {ap.target.location}</span>}
            <span>
              · {t('autopilot.wizard.lastRun')} {lastRun}
            </span>
            <span>
              · {t('autopilot.wizard.found')} {ap.totalFound}
            </span>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1.5 shrink-0">
          <Button
            onClick={onRun}
            disabled={running}
            className="flex items-center gap-1.5 rounded-lg bg-brand/10 px-2.5 py-1.5 text-[11px] font-medium text-brand-soft hover:bg-brand/20 transition-colors disabled:opacity-40 h-auto border-transparent"
          >
            {running ? <RotateCcw size={11} className="animate-spin" /> : <Play size={11} />}
            {running ? RUN_STATE_LABEL[runState] : t('autopilot.wizard.run')}
          </Button>
          {foundJobs.length > 0 && (
            <Button
              onClick={() => setShowFound((v) => !v)}
              aria-label={t('autopilot.foundJobs')}
              title={t('autopilot.foundJobs')}
              className={cn(
                'flex items-center gap-1 rounded-lg px-2 py-1.5 text-[11px] font-medium transition-colors h-auto border-transparent',
                showFound
                  ? 'bg-brand/15 text-brand-soft'
                  : 'bg-white/[0.04] text-foreground/50 hover:text-foreground/80'
              )}
            >
              <Briefcase size={11} />
              {foundJobs.length}
            </Button>
          )}
          <Button
            onClick={onTogglePause}
            className="rounded-lg p-1.5 text-foreground/40 hover:bg-white/[0.06] hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
          >
            {paused ? <Play size={13} /> : <Pause size={13} />}
          </Button>
          <Button
            onClick={onEdit}
            disabled={running}
            aria-label={t('autopilot.edit')}
            title={t('autopilot.edit')}
            className="rounded-lg p-1.5 text-foreground/40 hover:bg-white/[0.06] hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent disabled:opacity-40"
          >
            <Pencil size={13} />
          </Button>
          <Button
            onClick={() => setConfirmDelete(true)}
            aria-label={t('autopilot.delete')}
            title={t('autopilot.delete')}
            className="rounded-lg p-1.5 text-foreground/30 hover:bg-red-400/10 hover:text-red-400/70 transition-colors h-auto bg-transparent border-transparent"
          >
            <Trash2 size={13} />
          </Button>
        </div>
      </div>

      {/* Live step log — only visible while running */}
      <AnimatePresence>
        {running && stepLogs.length > 0 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <div className="rounded-lg bg-white/[0.03] border border-white/[0.05] px-3 py-2 space-y-1 max-h-32 overflow-y-auto">
              {stepLogs.map((log, i) => (
                <div key={i} className="flex items-start gap-2 text-[10px] leading-relaxed">
                  <span className="text-brand-soft/70 shrink-0 w-3 text-center">
                    {STEP_ICON[log.step] ?? '·'}
                  </span>
                  <span className="text-foreground/50 font-mono">{log.detail}</span>
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Found jobs from the most recent run */}
      <AnimatePresence>
        {showFound && foundJobs.length > 0 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={transition.fast}
            className="overflow-hidden"
          >
            <div className="overflow-hidden rounded-lg border border-white/[0.05] bg-white/[0.03]">
              <div className="flex items-center justify-between border-b border-white/[0.05] px-3 py-2">
                <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
                  {t('autopilot.foundJobs')} · {foundJobs.length}
                </span>
                <Button
                  variant="unstyled"
                  type="button"
                  onClick={() => setShowFound(false)}
                  aria-label={t('autopilot.collapse')}
                  title={t('autopilot.collapse')}
                  className="rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                >
                  <ChevronUp size={12} />
                </Button>
              </div>
              <div className="max-h-64 divide-y divide-white/[0.04] overflow-y-auto">
                {foundJobs.map((job, i) => (
                  <div
                    key={`${job.url}-${i}`}
                    className="flex items-center gap-2 px-3 py-2 transition-colors hover:bg-white/[0.03]"
                  >
                    <Button
                      variant="unstyled"
                      type="button"
                      onClick={() => void openExternal.mutate(job.url)}
                      title={t('autopilot.viewJob')}
                      className="flex min-w-0 flex-1 items-center gap-2 text-left"
                    >
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-[11px] text-foreground/80">
                            {job.title}
                          </span>
                          {job.isNew && (
                            <span className="shrink-0 rounded-full bg-brand/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-brand-soft">
                              {t('autopilot.badge.new')}
                            </span>
                          )}
                          {job.applied && (
                            <span className="flex shrink-0 items-center gap-0.5 rounded-full bg-emerald-400/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-emerald-300">
                              <Check size={8} /> {t('autopilot.badge.applied')}
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-1.5 text-[10px] text-foreground/40">
                          <span className="truncate">{job.company}</span>
                          {job.location && <span className="truncate">· {job.location}</span>}
                        </div>
                      </div>
                      {typeof job.score === 'number' && (
                        <span className="shrink-0 rounded bg-brand/10 px-1.5 py-0.5 text-[9px] text-brand-soft">
                          {Math.round(job.score)}%
                        </span>
                      )}
                      <ExternalLink size={11} className="shrink-0 text-foreground/25" />
                    </Button>
                    <Button
                      onClick={() => setApplyJob(job)}
                      title={t('autopilot.applyJob')}
                      className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-brand/10 px-2 py-1 text-[10px] font-medium text-brand-soft transition-colors hover:bg-brand/20 h-auto"
                    >
                      <Wand2 size={10} /> {t('autopilot.applyJob')}
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {applyJob && (
        <ApplyJobModal
          job={applyJob}
          resumeText={ap.resumeText}
          baseCoverLetter={ap.coverLetter}
          board={ap.target.board}
          onClose={() => setApplyJob(null)}
        />
      )}

      <ConfirmModal
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={() => {
          setConfirmDelete(false);
          onDelete();
        }}
        title={t('autopilot.deleteTitle')}
        description={t('autopilot.deleteDescription')}
        confirmText={t('autopilot.delete')}
        variant="danger"
      />
    </GlassCard>
  );
}
