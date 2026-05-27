import { AlertCircle, CheckCircle2, Send, ShieldAlert, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';

import { Button, GlassCard, TextArea, cn } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useApplyJob, useCancelJob, useJobEvents } from '@/services';

interface Posting {
  id: string;
  source: string;
  externalId: string;
  url: string;
  title: string;
  company: string;
  location?: string;
  remote?: boolean;
  description: string;
  postedAt?: number;
  capturedAt: number;
  interactions?: { interactionType: string }[];
}

interface ApplyStep {
  ts: number;
  stage: string;
  ok: boolean;
  note?: string;
  kind: 'step' | 'progress';
  p?: number;
}

interface ApplyDrawerProps {
  posting: Posting;
  onClose: () => void;
}

const APPLIABLE = new Set(['linkedin', 'indeed', 'greenhouse', 'workday', 'xing', 'glassdoor']);

export function ApplyDrawer({ posting, onClose }: ApplyDrawerProps) {
  const { t } = useTranslation();
  const [autoSubmit, setAutoSubmit] = useState(false);
  const [coverLetter, setCoverLetter] = useState('');
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [steps, setSteps] = useState<ApplyStep[]>([]);
  const [outcome, setOutcome] = useState<{ ok: boolean; submitted: boolean; note?: string } | null>(
    null
  );
  const jobRef = useRef<string | null>(null);
  const applyJob = useApplyJob();
  const cancelJobMutation = useCancelJob();

  jobRef.current = jobId;

  useJobEvents((raw: unknown) => {
    const ev = raw as { type: string; jobId: string; data?: unknown; ts: number };
    if (ev.jobId !== jobRef.current) return;
    if (ev.type === 'job.stream') {
      const data = ev.data as ApplyStep;
      setSteps((prev) =>
        [
          ...prev,
          {
            ts: ev.ts,
            stage: data.stage,
            ok: 'ok' in data ? data.ok : true,
            ...('note' in data && data.note ? { note: data.note } : {}),
            kind: data.kind,
            ...(data.kind === 'progress' ? { p: data.p } : {}),
          },
        ].slice(-40)
      );
    } else if (ev.type === 'job.completed') {
      const r = ev.data as { ok: boolean; submitted: boolean; note?: string };
      setOutcome(r);
      setRunning(false);
    } else if (ev.type === 'job.failed' || ev.type === 'job.cancelled') {
      setOutcome({ ok: false, submitted: false, note: String(ev.data ?? 'failed') });
      setRunning(false);
    }
  });

  const start = async () => {
    setSteps([]);
    setOutcome(null);
    setRunning(true);
    try {
      const res = (await applyJob.mutateAsync({
        board: posting.source,
        url: posting.url,
        ...(coverLetter.trim() ? { coverLetter: coverLetter.trim() } : {}),
        autoSubmit,
      })) as { jobId: string };
      jobRef.current = res.jobId;
      setJobId(res.jobId);
    } catch (err) {
      setOutcome({
        ok: false,
        submitted: false,
        note: err instanceof Error ? err.message : String(err),
      });
      setRunning(false);
    }
  };

  const cancel = async () => {
    if (jobId) await cancelJobMutation.mutateAsync(jobId);
  };

  const canApply = APPLIABLE.has(posting.source);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between border-b border-white/5 p-5">
        <div className="min-w-0">
          <div className="text-[10px] uppercase tracking-[0.3em] text-foreground/40">
            {t('jobs.applyDrawer.eyebrow')}
          </div>
          <div className="mt-1 truncate text-base font-medium text-foreground">{posting.title}</div>
          <div className="mt-0.5 truncate text-xs text-foreground/55">
            {posting.company} · {posting.source}
          </div>
        </div>
        <Button
          onClick={onClose}
          className="rounded-lg bg-white/5 p-1.5 text-foreground/60 hover:text-foreground h-auto border-transparent"
          aria-label={t('jobs.close')}
        >
          <X size={14} />
        </Button>
      </header>

      {!canApply ? (
        <div className="flex-1 p-5">
          <GlassCard tone="violet" highlight className="text-sm text-foreground/70">
            {t('jobs.applyNotSupported')}
          </GlassCard>
        </div>
      ) : (
        <>
          <div className="flex-1 overflow-y-auto p-5">
            {/* ToS warning */}
            <div className="mb-4 flex items-start gap-2 rounded-xl border border-amber-400/15 bg-amber-400/5 p-3 text-xs text-amber-200/90">
              <ShieldAlert size={14} className="mt-0.5 shrink-0" />
              <div>
                <div className="font-medium">{t('jobs.applyDrawer.warningTitle')}</div>
                <div className="mt-0.5 text-[11px] text-foreground/60">
                  {t('jobs.applyDrawer.warningBody')}
                </div>
              </div>
            </div>

            {/* Cover letter input */}
            <div className="mb-5">
              <label className="mb-2 block text-xs font-medium text-foreground/90">
                {t('jobs.applyDrawer.coverLetter')}
              </label>
              <TextArea
                value={coverLetter}
                onChange={(e) => setCoverLetter(e.target.value)}
                placeholder={t('jobs.applyDrawer.coverLetterPlaceholder')}
                disabled={running}
                rows={4}
                className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
              />
            </div>

            {/* Auto-submit toggle */}
            <div className="mb-5 flex items-center gap-2.5 rounded-xl bg-white/[0.02] p-3">
              <input
                type="checkbox"
                checked={autoSubmit}
                onChange={(e) => setAutoSubmit(e.target.checked)}
                disabled={running}
                className="h-4 w-4 accent-[var(--color-brand)] cursor-pointer"
              />
              <div
                className="flex-1 cursor-pointer"
                onClick={() => !running && setAutoSubmit(!autoSubmit)}
              >
                <div className="text-xs font-medium text-foreground/90">
                  {t('jobs.applyDrawer.autoSubmit')}
                </div>
                <div className="text-[11px] text-foreground/45">
                  {t('jobs.applyDrawer.autoSubmitHint')}
                </div>
              </div>
            </div>

            {/* Step feed */}
            {steps.length > 0 && (
              <div className="mb-4">
                <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/40">
                  {t('jobs.applyDrawer.steps')}
                </div>
                <div className="space-y-1.5">
                  <AnimatePresence initial={false}>
                    {steps.map((s, i) => (
                      <StepRow key={`${s.ts}-${i}`} step={s} />
                    ))}
                  </AnimatePresence>
                </div>
              </div>
            )}

            {outcome && (
              <GlassCard
                tone={outcome.ok ? 'indigo' : 'graphite'}
                highlight
                glow={outcome.ok}
                className="mt-3"
              >
                <div className="flex items-center gap-2 text-sm">
                  {outcome.ok ? (
                    <CheckCircle2 size={14} className="text-emerald-300" />
                  ) : (
                    <AlertCircle size={14} className="text-amber-300" />
                  )}
                  <span className="font-medium">
                    {outcome.submitted
                      ? t('jobs.applyDrawer.submitted')
                      : outcome.ok
                        ? t('jobs.applyDrawer.reviewPending')
                        : t('jobs.applyDrawer.failed')}
                  </span>
                </div>
                {outcome.note && (
                  <div className="mt-1 text-[11px] text-foreground/55">{outcome.note}</div>
                )}
              </GlassCard>
            )}
          </div>

          <footer className="flex items-center justify-end gap-2 border-t border-white/5 p-4">
            {running && (
              <Button size="sm" variant="ghost" onClick={() => void cancel()}>
                {t('jobs.applyDrawer.cancel')}
              </Button>
            )}
            <Button
              size="md"
              variant={running ? 'ghost' : 'glass'}
              onClick={() => void start()}
              disabled={running}
              loading={running}
              className="transition-all duration-150 ease-out"
            >
              {!running && <Send size={14} />}
              {running ? t('jobs.applyDrawer.running') : t('jobs.applyDrawer.start')}
            </Button>
          </footer>
        </>
      )}
    </div>
  );
}

function StepRow({ step }: { step: ApplyStep }) {
  if (step.kind === 'progress') {
    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="rounded-md bg-white/[0.02] px-2.5 py-1.5"
      >
        <div className="mb-1 flex items-center justify-between text-[11px]">
          <span className="text-foreground/65">{step.stage}</span>
          <span className="text-foreground/45">{Math.round((step.p ?? 0) * 100)}%</span>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-white/5">
          <div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
            style={{ width: `${(step.p ?? 0) * 100}%` }}
          />
        </div>
      </motion.div>
    );
  }
  return (
    <motion.div
      initial={{ opacity: 0, x: 6 }}
      animate={{ opacity: 1, x: 0 }}
      className="flex items-start gap-2 text-[12px]"
    >
      <span
        className={cn(
          'mt-1.5 h-1.5 w-1.5 flex-shrink-0 rounded-full shadow-[0_0_8px_currentColor]',
          step.ok ? 'bg-emerald-400' : 'bg-amber-400'
        )}
      />
      <div className="flex-1">
        <span className="text-foreground/85">{step.stage}</span>
        {step.note && <div className="text-[11px] text-foreground/45">{step.note}</div>}
      </div>
    </motion.div>
  );
}
