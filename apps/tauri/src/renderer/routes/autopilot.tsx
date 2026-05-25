import {
  AlertCircle,
  BookOpen,
  Calendar,
  Check,
  ChevronLeft,
  ChevronRight,
  Filter,
  Pause,
  Play,
  Plus,
  RotateCcw,
  Send,
  Target,
  Trash2,
  X,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useReducer, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type {
  Autopilot,
  AutopilotAction,
  AutopilotSchedule,
  AutopilotStepEvent,
  JobPreferences,
} from '@ajh/shared';
import { Button, GlassCard } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { StepAction } from '@/features/autopilot/components/wizard-steps/StepAction';
import { StepFilter } from '@/features/autopilot/components/wizard-steps/StepFilter';
import { StepSchedule } from '@/features/autopilot/components/wizard-steps/StepSchedule';
import { StepTarget } from '@/features/autopilot/components/wizard-steps/StepTarget';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition as machineTransition } from '@/lib/machine';
import {
  autopilotRunMachine,
  type AutopilotRunState,
  RUN_STATE_LABEL,
  stepToEvent,
} from '@/lib/machines/autopilot-run.machine';
import { transition } from '@/lib/motion';
import {
  useAutopilots,
  useAutopilotStepEvents,
  useCreateAutopilot,
  useJobPreferences,
  usePauseAutopilot,
  useRemoveAutopilot,
  useResumeAutopilot,
  useRunAutopilot,
} from '@/services';

export const Route = createFileRoute('/autopilot')({ component: AutopilotPage });

// ─── Wizard state ─────────────────────────────────────────────────────────────

export interface WizardState {
  name: string;
  // Step 1 — Target
  board: string;
  query: string;
  location: string;
  workType: 'remote' | 'hybrid' | 'on-site' | 'any';
  pages: number;
  dateFilter: string;
  // Step 2 — Filter
  minMatchScore: number;
  keywords: string;
  excludeKeywords: string;
  resumeText: string;
  // Step 3 — Action
  action: AutopilotAction;
  coverLetter: string;
  autoSubmit: boolean;
  // Step 4 — Schedule
  schedule: AutopilotSchedule;
}

function buildDefaults(jobPrefs?: JobPreferences): WizardState {
  const validWorkType = ['remote', 'hybrid', 'on-site', 'any'] as const;
  return {
    name: '',
    board: 'linkedin',
    query: '',
    location: jobPrefs?.location ?? '',
    workType: validWorkType.includes(jobPrefs?.remote as (typeof validWorkType)[number])
      ? (jobPrefs?.remote as WizardState['workType'])
      : 'any',
    pages: 2,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: jobPrefs?.techStack?.map((t) => t.name).join(', ') ?? '',
    excludeKeywords: '',
    resumeText: '',
    action: 'save',
    coverLetter: '',
    autoSubmit: false,
    schedule: 'daily',
  };
}

const STEPS = ['Target', 'Filter', 'Action', 'Schedule'] as const;

export type SetFn = <K extends keyof WizardState>(k: K, v: WizardState[K]) => void;
export type Prefilled = { location: boolean; keywords: boolean };

const _ACTION_OPTIONS: {
  id: AutopilotAction;
  label: string;
  desc: string;
  icon: React.ElementType;
  color: string;
}[] = [
  {
    id: 'save',
    label: 'Save only',
    desc: 'Collect matching jobs to your list. You review and apply manually.',
    icon: BookOpen,
    color: 'text-blue-400',
  },
  {
    id: 'review',
    label: 'Apply & review',
    desc: 'Start the application but stop before submitting. You confirm each one.',
    icon: Send,
    color: 'text-amber-400',
  },
  {
    id: 'auto_apply',
    label: 'Auto-apply',
    desc: 'Submit applications automatically. Use with care — cannot be undone.',
    icon: Zap,
    color: 'text-brand-soft',
  },
];

const _SCHEDULE_OPTIONS: { id: AutopilotSchedule; label: string; desc: string }[] = [
  { id: 'manual', label: 'Manual only', desc: 'Run only when you press the Run button.' },
  { id: 'hourly', label: 'Every hour', desc: 'Runs every 60 minutes while the app is open.' },
  { id: 'twice_daily', label: 'Twice a day', desc: 'Runs at startup and again 12 hours later.' },
  { id: 'daily', label: 'Once a day', desc: 'Runs once per day while the app is open.' },
];

// ─── Page ─────────────────────────────────────────────────────────────────────

interface StepLog {
  step: string;
  detail: string;
  ts: number;
}

type RunStateMap = Record<string, AutopilotRunState>;
type StepLogMap = Record<string, StepLog[]>;

function AutopilotPage() {
  const { t } = useTranslation();
  const { data: autopilotList = [], isLoading: loading } = useAutopilots();
  const autopilots = autopilotList as Autopilot[];
  const [creating, setCreating] = useState(false);
  const [runStates, setRunStates] = useReducer(
    (prev: RunStateMap, patch: Partial<RunStateMap>): RunStateMap =>
      ({ ...prev, ...patch }) as RunStateMap,
    {} as RunStateMap
  );
  const [stepLogs, setStepLogs] = useReducer(
    (prev: StepLogMap, patch: Partial<StepLogMap>): StepLogMap =>
      ({ ...prev, ...patch }) as StepLogMap,
    {} as StepLogMap
  );
  const [error, setError] = useState<string | null>(null);

  const runAutopilot = useRunAutopilot();
  const pauseAutopilot = usePauseAutopilot();
  const resumeAutopilot = useResumeAutopilot();
  const removeAutopilot = useRemoveAutopilot();

  const handleStep = useCallback(
    (event: AutopilotStepEvent) => {
      const ev = stepToEvent(event.step);
      if (ev) {
        setRunStates({
          [event.autopilotId]: machineTransition(
            autopilotRunMachine,
            runStates[event.autopilotId] ?? 'idle',
            ev
          ),
        });
      }
      setStepLogs({
        [event.autopilotId]: [
          ...(stepLogs[event.autopilotId] ?? []).slice(-49),
          { step: event.step, detail: event.detail, ts: Date.now() },
        ],
      });
    },
    [runStates, stepLogs]
  );

  useAutopilotStepEvents(handleStep);

  const handleRun = async (id: string) => {
    setRunStates({ [id]: 'scraping' });
    setStepLogs({ [id]: [] });
    try {
      await runAutopilot.mutateAsync(id);
      setRunStates({ [id]: 'done' });
    } catch (err) {
      setRunStates({ [id]: 'error' });
      setError(err instanceof Error ? err.message : t('autopilot.wizard.runFailed'));
    }
  };

  const handleTogglePause = async (ap: Autopilot) => {
    if (ap.status === 'paused') await resumeAutopilot.mutateAsync(ap._id);
    else await pauseAutopilot.mutateAsync(ap._id);
  };

  const handleDelete = async (id: string) => {
    await removeAutopilot.mutateAsync(id);
  };

  const handleCreated = (_ap: Autopilot) => {
    setCreating(false);
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-5 shrink-0">
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-brand/15">
              <Zap size={15} className="text-brand-soft" />
            </div>
            <div>
              <h1 className="text-base font-semibold text-foreground/90">{t('autopilot.title')}</h1>
              <p className="text-[11px] text-foreground/40">{t('autopilot.subtitle')}</p>
            </div>
          </div>
          <Button
            variant="glass"
            size="sm"
            onClick={() => setCreating(true)}
            className="transition-all duration-150 ease-out"
          >
            <Plus size={13} /> {t('autopilot.newAutopilot')}
          </Button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-8 py-6">
          {error && (
            <div className="mb-4 flex items-center gap-2 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-3 text-xs text-red-300/80">
              <AlertCircle size={12} /> {error}
              <Button
                onClick={() => setError(null)}
                className="ml-auto h-auto bg-transparent border-transparent p-0"
              >
                <X size={11} />
              </Button>
            </div>
          )}

          {loading ? (
            <div className="flex items-center justify-center h-40 text-foreground/30 text-sm">
              {t('autopilot.loading')}
            </div>
          ) : autopilots.length === 0 ? (
            <EmptyState onNew={() => setCreating(true)} />
          ) : (
            <div className="space-y-3">
              {autopilots.map((ap) => {
                const runState = runStates[ap._id] ?? 'idle';
                return (
                  <AutopilotCard
                    key={ap._id}
                    autopilot={ap}
                    runState={runState}
                    stepLogs={stepLogs[ap._id] ?? []}
                    onRun={() => void handleRun(ap._id)}
                    onTogglePause={() => void handleTogglePause(ap)}
                    onDelete={() => void handleDelete(ap._id)}
                  />
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Creation wizard overlay */}
      <AnimatePresence>
        {creating && <CreationWizard onDone={handleCreated} onCancel={() => setCreating(false)} />}
      </AnimatePresence>
    </PageTransition>
  );
}

// ─── Autopilot card ───────────────────────────────────────────────────────────

const STEP_ICON: Record<string, string> = {
  scrape_start: '⟳',
  scrape_done: '✓',
  rank_done: '★',
  apply_start: '→',
  apply_done: '✓',
  cancelled: '⊘',
  complete: '✓',
};

function AutopilotCard({
  autopilot: ap,
  runState,
  stepLogs,
  onRun,
  onTogglePause,
  onDelete,
}: {
  autopilot: Autopilot;
  runState: AutopilotRunState;
  stepLogs: StepLog[];
  onRun(): void;
  onTogglePause(): void;
  onDelete(): void;
}) {
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

// ─── Empty state ──────────────────────────────────────────────────────────────

function EmptyState({ onNew }: { onNew(): void }) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center gap-6 py-20 text-center">
      <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/20">
        <Zap size={36} className="text-brand-soft/60" />
      </div>
      <div>
        <div className="text-lg font-semibold text-foreground/50">{t('autopilot.empty.title')}</div>
        <div className="mt-1 text-sm text-foreground/30 max-w-sm">
          {t('autopilot.empty.description')}
        </div>
      </div>
      <div className="flex flex-col gap-2 text-left">
        {[
          { icon: Target, text: t('autopilot.empty.step1') },
          { icon: Filter, text: t('autopilot.empty.step2') },
          { icon: Send, text: t('autopilot.empty.step3') },
          { icon: Calendar, text: t('autopilot.empty.step4') },
        ].map(({ icon: Icon, text }, i) => (
          <div key={i} className="flex items-center gap-2.5 text-xs text-foreground/35">
            <Icon size={12} className="text-brand-soft/50 shrink-0" /> {text}
          </div>
        ))}
      </div>
      <Button
        variant="glass"
        size="md"
        onClick={onNew}
        className="transition-all duration-150 ease-out px-6 gap-2"
      >
        <Plus size={14} /> {t('autopilot.empty.createFirst')}
      </Button>
    </div>
  );
}

// ─── Creation wizard ──────────────────────────────────────────────────────────

function CreationWizard({ onDone, onCancel }: { onDone(ap: Autopilot): void; onCancel(): void }) {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();

  const [step, setStep] = useState(0);
  const [form, setForm] = useState<WizardState>(() => buildDefaults(jobPrefs));
  const [error, setError] = useState<string | null>(null);
  const createAutopilot = useCreateAutopilot();
  const saving = createAutopilot.isPending;

  // Track which fields were pre-filled so we can show a hint
  const prefilledFields = {
    location: !!jobPrefs?.location,
    keywords: (jobPrefs?.techStack?.length ?? 0) > 0,
  };

  const set: SetFn = <K extends keyof WizardState>(k: K, v: WizardState[K]) =>
    setForm((p) => ({ ...p, [k]: v }));

  const canNext = () => {
    if (step === 0)
      return form.board && form.query.trim().length > 1 && form.name.trim().length > 1;
    return true;
  };

  const save = async () => {
    setError(null);
    try {
      const ap = (await createAutopilot.mutateAsync({
        name: form.name,
        target: {
          board: form.board,
          query: form.query,
          location: form.location || undefined,
          workType: form.workType !== 'any' ? form.workType : undefined,
          pages: form.pages,
          dateFilter: form.dateFilter || undefined,
        },
        filter: {
          minMatchScore: form.minMatchScore,
          keywords: form.keywords
            ? form.keywords
                .split(',')
                .map((s) => s.trim())
                .filter(Boolean)
            : undefined,
          excludeKeywords: form.excludeKeywords
            ? form.excludeKeywords
                .split(',')
                .map((s) => s.trim())
                .filter(Boolean)
            : undefined,
        },
        resumeText: form.resumeText || undefined,
        action: form.action,
        coverLetter: form.coverLetter || undefined,
        autoSubmit: form.autoSubmit,
        schedule: form.schedule,
      })) as Autopilot;
      onDone(ap);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('autopilot.wizard.createFailed'));
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={transition.fast}
      className="absolute inset-0 z-[var(--z-modal)] flex items-center justify-center"
    >
      {/*
       * BACKDROP LAYERING — reading order = bottom to top
       *
       * 1. blur layer     — destroys shape detail
       * 2. crush layer    — kills remaining contrast/readability
       * 3. bokeh blobs    — adds ambient atmospheric color
       * 4. vignette       — pulls focus to center
       * 5. modal panel    — the actual foreground
       */}

      {/* 1 — heavy blur. backdrop-filter alone preserves contrast,
              so we follow it with a dark crush layer. */}
      <div
        className="absolute inset-0"
        style={{
          backdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
          WebkitBackdropFilter: 'blur(64px) saturate(120%) brightness(0.6)',
        }}
      />

      {/* 2 — contrast crush: semi-opaque dark tint that makes the
              blurred background unreadable without going full black. */}
      <div className="absolute inset-0 bg-background/70" />

      {/* 3 — bokeh blobs: soft ambient color on top of the dark crush. */}
      <div className="absolute inset-0 pointer-events-none overflow-hidden">
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob1}
          className="absolute"
          style={{
            top: '-5%',
            left: '5%',
            width: 500,
            height: 500,
            background: 'radial-gradient(circle, rgba(168,85,247,0.18) 0%, transparent 65%)',
            filter: 'blur(48px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob2}
          className="absolute"
          style={{
            bottom: '0%',
            right: '5%',
            width: 420,
            height: 420,
            background: 'radial-gradient(circle, rgba(79,70,229,0.15) 0%, transparent 65%)',
            filter: 'blur(56px)',
          }}
        />
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.blob3}
          className="absolute"
          style={{
            top: '40%',
            left: '40%',
            width: 300,
            height: 300,
            background: 'radial-gradient(circle, rgba(192,38,211,0.07) 0%, transparent 65%)',
            filter: 'blur(72px)',
          }}
        />
      </div>

      {/* 4 — vignette: darkens screen edges, naturally centres the eye. */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            'radial-gradient(ellipse 80% 80% at 50% 50%, transparent 35%, rgba(0,0,0,0.65) 100%)',
        }}
      />

      {/* 5 — modal glass panel */}
      <motion.div
        initial={{ opacity: 0, scale: 0.96, y: 14 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.96, y: 14 }}
        transition={transition.relaxed}
        style={{
          /* Denser, milkier glass — not transparent tinted window */
          background: 'rgba(14, 14, 26, 0.88)',
          backdropFilter: 'blur(32px) saturate(180%)',
          WebkitBackdropFilter: 'blur(32px) saturate(180%)',
          border: '1px solid rgba(255,255,255,0.13)',
          boxShadow: [
            /* purple accent ring */
            '0 0 0 1px rgba(168,85,247,0.15)',
            /* deep lift shadow */
            '0 40px 100px rgba(0,0,0,0.75)',
            '0 12px 40px rgba(0,0,0,0.5)',
            /* top edge highlight — the classic Apple glass line */
            'inset 0 1px 0 rgba(255,255,255,0.13)',
            /* bottom edge dim */
            'inset 0 -1px 0 rgba(0,0,0,0.3)',
          ].join(', '),
        }}
        className="relative w-full max-w-xl rounded-2xl overflow-hidden"
      >
        {/* Wizard header */}
        <div className="flex items-center justify-between border-white/[0.1] px-6 py-4">
          <div className="flex items-center gap-2">
            <Zap size={14} className="text-brand-soft" />
            <span className="text-sm font-semibold text-foreground/80">
              {t('autopilot.wizard.title')}
            </span>
          </div>
          <Button
            onClick={onCancel}
            className="text-foreground/30 hover:text-foreground/60 transition-colors h-auto bg-transparent border-transparent p-0"
          >
            <X size={16} />
          </Button>
        </div>

        {/* Step indicators */}
        <div className="flex items-center gap-0">
          {STEPS.map((s, i) => (
            <div
              key={s}
              className={cn(
                'flex-1 flex items-center justify-center gap-1.5 py-2.5 text-[11px] font-medium transition-colors',
                i === step
                  ? 'border-brand text-brand-soft'
                  : i < step
                    ? 'border-emerald-400/50 text-foreground/40'
                    : 'border-transparent text-foreground/25'
              )}
            >
              {i < step ? (
                <Check size={10} className="text-emerald-400" />
              ) : (
                <span className="h-4 w-4 rounded-full border border-current flex items-center justify-center text-[9px]">
                  {i + 1}
                </span>
              )}
              {t(`autopilot.wizard.steps.${i}`)}
            </div>
          ))}
        </div>

        {/* Step content */}
        <div className="px-6 py-6 min-h-[320px]">
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 16 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -16 }}
              transition={transition.normal}
            >
              {step === 0 && <StepTarget form={form} set={set} prefilled={prefilledFields} />}
              {step === 1 && <StepFilter form={form} set={set} prefilled={prefilledFields} />}
              {step === 2 && <StepAction form={form} set={set} />}
              {step === 3 && <StepSchedule form={form} set={set} />}
            </motion.div>
          </AnimatePresence>
        </div>

        {error && (
          <div className="mx-6 mb-4 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-xs text-red-300/80 flex items-center gap-2">
            <AlertCircle size={11} /> {error}
          </div>
        )}

        {/* Wizard footer */}
        <div className="flex items-center justify-between border-t border-white/[0.1] px-6 py-4">
          <Button
            onClick={() => (step > 0 ? setStep((s) => s - 1) : onCancel())}
            className="flex items-center gap-1.5 text-xs text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
          >
            <ChevronLeft size={13} />{' '}
            {step === 0 ? t('autopilot.wizard.cancel') : t('autopilot.wizard.back')}
          </Button>
          {step < STEPS.length - 1 ? (
            <Button
              variant="glass"
              size="sm"
              disabled={!canNext()}
              onClick={() => setStep((s) => s + 1)}
              className="transition-all duration-150 ease-out"
            >
              {t('autopilot.wizard.next')} <ChevronRight size={13} />
            </Button>
          ) : (
            <Button
              variant="glass"
              size="sm"
              loading={saving}
              onClick={() => void save()}
              className="transition-all duration-150 ease-out"
            >
              {!saving && <Zap size={13} />} {t('autopilot.wizard.create')}
            </Button>
          )}
        </div>
      </motion.div>
    </motion.div>
  );
}

// ─── Wizard steps ──────────────────────────────────────────────────────────────
// Wizard step components extracted to features/autopilot/components/wizard-steps/
