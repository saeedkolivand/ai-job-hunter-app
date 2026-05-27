import { Pause, Play, RotateCcw, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import type { Autopilot } from '@ajh/shared';
import { Button, cn, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { type AutopilotRunState, RUN_STATE_LABEL } from '@/lib/machines/autopilot-run.machine';

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
  onDelete(): void;
}

const STEP_ICON: Record<string, string> = {
  scrape_start: '⟳',
  scrape_done: '✓',
  rank_done: '★',
  apply_start: '→',
  apply_done: '✓',
  cancelled: '⊘',
  complete: '✓',
};

export function AutopilotCard({
  autopilot: ap,
  runState,
  stepLogs,
  onRun,
  onTogglePause,
  onDelete,
}: AutopilotCardProps) {
  const paused = ap.status === 'paused';
  const running = runState === 'scraping' || runState === 'ranking' || runState === 'applying';
  const { t } = useTranslation();
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
              {ap.action.replace('_', ' ')}
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
              · {t('autopilot.wizard.found')} {ap.totalFound} · {t('autopilot.wizard.applied')}{' '}
              {ap.totalApplied}
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
          <Button
            onClick={onTogglePause}
            className="rounded-lg p-1.5 text-foreground/40 hover:bg-white/[0.06] hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
          >
            {paused ? <Play size={13} /> : <Pause size={13} />}
          </Button>
          <Button
            onClick={onDelete}
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
            transition={{ duration: 0.2 }}
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
    </GlassCard>
  );
}
