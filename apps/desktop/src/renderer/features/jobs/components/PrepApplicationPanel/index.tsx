import { CheckCircle2, Circle, CircleMinus, Loader2, Sparkles, Square, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';

import type { AgentConfirmPayload, AgentStepEvent, JobEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, ErrorState, GlassCard, ModalShell, StreamingText, Tag } from '@ajh/ui';

import { AgentConfirm } from '@/features/jobs/components/AgentConfirm';
import type { Posting } from '@/features/jobs/types';
import { useMachine } from '@/hooks/use-machine';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { agentRunMachine, stepToEvent } from '@/lib/machines/agent-run.machine';
import {
  useAgentRun,
  useAgentStepEvents,
  useCancelJob,
  useGenerateConfig,
  useJobEvents,
} from '@/services';

interface AgentRunResult {
  finalText: string;
  steps: number;
  stoppedReason: string;
}

type ChecklistKey = 'research' | 'match' | 'draft' | 'questions' | 'propose';

interface ChecklistItem {
  key: ChecklistKey;
  /** Tool name to match against a turn's `tools[]`; `null` selects the
   *  terminal `kind: 'proposal'` step instead (there's no tool for it). */
  tool: string | null;
}

// The terminal "propose" row is included so the checklist never silently gaps
// after the last tool row — its body text is intentionally NOT rendered here
// (see the `proposalStep` GlassCard below): showing the same streamed text in
// both places would be the double-render the reviewer already confirmed is
// correctly avoided for the proposal card itself, so this row stays
// status-only for consistency.
const CHECKLIST: ChecklistItem[] = [
  { key: 'research', tool: 'research_company' },
  { key: 'match', tool: 'match_resume' },
  { key: 'draft', tool: 'draft_cover_letter' },
  { key: 'questions', tool: 'suggest_interview_questions' },
  { key: 'propose', tool: null },
];

/**
 * Rust `StoppedReason` (`#[serde(rename_all = "snake_case")]`) → the
 * `jobs.prep.stopped.*` key suffix.
 */
const STOPPED_SUFFIX: Record<
  string,
  'done' | 'maxSteps' | 'maxTokens' | 'cancelled' | 'truncated' | 'budgeted'
> = {
  done: 'done',
  max_steps: 'maxSteps',
  max_tokens: 'maxTokens',
  cancelled: 'cancelled',
  truncated: 'truncated',
  budgeted: 'budgeted',
};

/**
 * Last step in the log matching `item` (a turn can revisit the same tool, or
 * — rarely — a single turn's `tools[]` can name more than one recognized
 * tool at once; in that case the same step is the "last match" for both
 * checklist rows and both rows show that turn's identical text, which is an
 * acceptable simplification for a narration checklist).
 */
function findLastMatch(steps: AgentStepEvent[], item: ChecklistItem): AgentStepEvent | undefined {
  for (let i = steps.length - 1; i >= 0; i--) {
    const s = steps[i];
    if (!s) continue;
    if (item.tool === null ? s.kind === 'proposal' : s.tools.includes(item.tool)) return s;
  }
  return undefined;
}

/**
 * "Prep this application" trigger + modal for a single job {@link Posting}.
 * Runs the agentic `agent.run` flow (Phase 2 — display-only, no write executes)
 * and streams its steps as a live checklist: research → match → draft →
 * questions → propose. Provider/model/résumé mirror the same selection
 * `ai_generate`/company-research already use.
 *
 * Capability gate: today there's no renderer-visible "does this provider
 * support tool calls" signal, so a non-tool-capable provider/model is only
 * caught backend-side (`agent_run` fails with a clear message, surfaced via
 * `job.failed`). A future `supports_tools` flag plumbed through
 * `useGenerateConfig`/the provider list would let this gate proactively
 * instead of round-tripping to find out — deferred, not built here.
 */
export function PrepApplicationPanel({ posting }: { posting: Posting }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const resumeId = useDefaultResumeId();
  const { provider, model, baseUrl } = useGenerateConfig();
  const canRun = !!resumeId && !!provider && !!model;

  const [machineState, send] = useMachine(agentRunMachine, 'idle');
  const [steps, setSteps] = useState<AgentStepEvent[]>([]);
  const [result, setResult] = useState<AgentRunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stopRequested, setStopRequested] = useState(false);
  const [runJobId, setRunJobId] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState('');
  // The one Write action currently suspended awaiting the user's decision
  // (Phase 3's confirm gate). Cleared on resolve, on a fresh run, or once the
  // run itself terminates (nothing left to confirm).
  const [pendingConfirm, setPendingConfirm] = useState<AgentConfirmPayload | null>(null);
  // Belt-and-suspenders alongside the `event.jobId === runJobId` check: guards
  // the brief window before `runJobId` is set (see `handleStep`) and gives
  // `start()` an instant (pre-render) "is a run already active" read so a
  // fast double-click can't spawn two concurrent runs.
  const activeRef = useRef(false);
  const prevStatusRef = useRef<Partial<Record<ChecklistKey, string>>>({});
  // A stable in-modal focus target: `AgentConfirm` (and its own heading) fully
  // unmounts once a confirm resolves, so focus needs somewhere durable to land
  // on — the modal title, which is always rendered — rather than falling
  // through to <body> when the just-clicked Approve/Deny button disappears.
  const modalTitleRef = useRef<HTMLSpanElement>(null);

  const runAgent = useAgentRun();
  const cancelJob = useCancelJob();

  const isBusy =
    machineState !== 'idle' &&
    machineState !== 'done' &&
    machineState !== 'error' &&
    machineState !== 'cancelled';
  const isTerminalRun = machineState === 'error' || machineState === 'cancelled';
  // The machine accepts START from every terminal state (including `done`),
  // so a successful run also gets a re-run affordance in the footer — not
  // just error/cancelled. Kept distinct from `isTerminalRun` because the
  // "interrupted" checklist-row treatment below must NOT apply to `done`
  // (a successful run's rows really did finish).
  const canRunAgain = isTerminalRun || machineState === 'done';

  const handleStep = useCallback(
    (event: AgentStepEvent) => {
      // `runJobId` is set once `agent.run`'s IPC round-trip resolves; a real
      // model turn (what actually produces a step) takes far longer than
      // that round-trip, so this is not a real race in practice.
      if (!activeRef.current || event.jobId !== runJobId) return;
      setSteps((prev) => [...prev, event]);
      if (event.kind === 'confirm_request' && event.confirm) {
        setPendingConfirm(event.confirm);
        // Piggyback on the existing milestone live region rather than adding a
        // second aria-live source — one announcement per suspend, not per token.
        setAnnouncement(t('jobs.prep.confirm.announcement'));
      } else {
        // Any other step for THIS run supersedes a prior pending confirm: the
        // backend's 300s CONFIRM_TIMEOUT denies-and-resumes server-side, so a
        // `turn`/`proposal` step can legitimately follow without the renderer
        // ever hearing a resolve. Clear the stale card rather than let it
        // linger next to fresh narration — fail-closed, since a late
        // Approve/Deny on that callId would return `{ ok: false }` anyway.
        setPendingConfirm(null);
      }
      const ev = stepToEvent(event);
      if (ev) send(ev);
    },
    [runJobId, send, t]
  );
  useAgentStepEvents(handleStep);

  const handleJobEvent = useCallback(
    (event: JobEvent) => {
      if (!activeRef.current || event.jobId !== runJobId) return;
      if (event.type === 'job.completed') {
        activeRef.current = false;
        setPendingConfirm(null);
        setResult(event.data as AgentRunResult);
        send('COMPLETE');
      } else if (event.type === 'job.failed') {
        activeRef.current = false;
        setPendingConfirm(null);
        setError(typeof event.data === 'string' ? event.data : t('jobs.prep.runFailed'));
        send('ERROR');
      } else if (event.type === 'job.cancelled') {
        // A deliberate Stop is not a failure — its own state/copy, not ERROR.
        // Cancelling a suspended run resolves its pending confirm server-side
        // too (see `AgentGate::remove`) — nothing left here to act on.
        activeRef.current = false;
        setPendingConfirm(null);
        send('CANCEL');
      }
    },
    [runJobId, send, t]
  );
  useJobEvents(handleJobEvent);

  /** `<AgentConfirm>` only calls this once the resolve actually took effect
   *  (`{ ok: true }`) — an `{ ok: false }` stays on-screen showing its own
   *  "no longer available" copy instead of silently vanishing. */
  const handleConfirmResolved = useCallback(
    (event: 'APPROVE' | 'DENY') => {
      setPendingConfirm(null);
      // `AgentConfirm` (and the button that was just clicked) is about to
      // unmount — re-park focus on the modal title, which is unaffected by
      // that removal and always present while the modal is open.
      modalTitleRef.current?.focus();
      send(event);
    },
    [send]
  );

  const start = useCallback(async () => {
    // isBusy/activeRef re-entrancy guard: a terminal state (error/cancelled/
    // done) now accepts START directly (see agentRunMachine), so Retry must
    // itself refuse to fire while a run is already active.
    if (!canRun || !resumeId || isBusy || activeRef.current) return;
    setSteps([]);
    setResult(null);
    setError(null);
    setStopRequested(false);
    setRunJobId(null);
    setAnnouncement('');
    setPendingConfirm(null);
    prevStatusRef.current = {};
    activeRef.current = true;
    send('START');
    try {
      const { jobId } = await runAgent.mutateAsync({
        resumeId,
        jobId: posting.id,
        provider,
        model,
        baseUrl,
      });
      setRunJobId(jobId);
    } catch (err) {
      activeRef.current = false;
      setError(err instanceof Error ? err.message : t('jobs.prep.runFailed'));
      send('ERROR');
    }
  }, [canRun, resumeId, isBusy, runAgent, posting.id, provider, model, baseUrl, send, t]);

  const stop = () => {
    if (!runJobId) return;
    setStopRequested(true);
    void cancelJob.mutateAsync(runJobId).catch(() => {});
  };

  const rows = CHECKLIST.map((item) => {
    const match = findLastMatch(steps, item);
    const isLatest = !!match && steps[steps.length - 1] === match;
    const status: 'pending' | 'active' | 'done' | 'interrupted' = !match
      ? 'pending'
      : isLatest && isBusy
        ? 'active'
        : isLatest && isTerminalRun
          ? 'interrupted' // was mid-flight when the run stopped/errored — never actually finished
          : 'done';
    return { item, match, status };
  });

  // Live-region milestone announcer: announces a step's status change ONCE
  // (not per streamed character) — the raw StreamingText prose stays outside
  // any aria-live region so screen readers aren't re-triggered on every token.
  // `rows` is recomputed fresh every render, so this effect re-runs on every
  // render too; the `prevStatusRef` guard makes that a cheap no-op except on
  // an actual status change, so no memoization is needed to keep it correct.
  useEffect(() => {
    for (const { item, status } of rows) {
      if (prevStatusRef.current[item.key] === status) continue;
      prevStatusRef.current[item.key] = status;
      if (status === 'pending') continue; // no-op transition, nothing to announce
      const label = t(`jobs.prep.steps.${item.key}`);
      const statusLabel = t(`jobs.prep.status.${status}`);
      setAnnouncement(`${label} — ${statusLabel}`);
    }
  }, [rows, t]);

  const proposalStep = steps.find((s) => s.kind === 'proposal');
  const stoppedSuffix = result ? (STOPPED_SUFFIX[result.stoppedReason] ?? 'done') : 'done';

  const showEmpty = machineState === 'idle' && steps.length === 0;
  const showStarting = isBusy && steps.length === 0;
  const showLog = steps.length > 0;
  const showFooter = (isBusy && !!runJobId) || canRunAgain;

  return (
    <>
      <Button
        variant="glass"
        onClick={() => setOpen(true)}
        title={t('jobs.prep.triggerHint')}
        className="shrink-0 gap-1.5 text-brand-soft"
      >
        <Sparkles size={13} /> {t('jobs.prep.trigger')}
      </Button>

      <ModalShell
        open={open}
        onClose={() => setOpen(false)}
        maxWidth="max-w-lg"
        ariaLabelledby="prep-application-modal-title"
        header={
          <div className="flex items-start justify-between gap-3 border-b border-[var(--border-soft)] px-5 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Sparkles size={14} className="shrink-0 text-brand-soft" />
                <span
                  ref={modalTitleRef}
                  id="prep-application-modal-title"
                  tabIndex={-1}
                  className="truncate text-sm font-semibold text-foreground/85 outline-none"
                >
                  {t('jobs.prep.modalTitle')}
                </span>
              </div>
              <div className="mt-0.5 truncate text-[11px] text-foreground/40">
                {posting.title} · {posting.company}
              </div>
            </div>
            <Button
              onClick={() => setOpen(false)}
              aria-label={t('jobs.prep.close')}
              className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
            >
              <X size={16} />
            </Button>
          </div>
        }
        footer={
          showFooter ? (
            <div className="flex items-center gap-2 border-t border-[var(--border-soft)] px-5 py-3">
              {isBusy && runJobId && (
                <Button
                  variant="glass"
                  onClick={stop}
                  disabled={stopRequested}
                  title={t('jobs.prep.stopHint')}
                  className="w-full justify-center gap-1.5 text-foreground/60"
                >
                  <Square size={11} />{' '}
                  {stopRequested ? t('jobs.prep.stopping') : t('jobs.prep.stop')}
                </Button>
              )}
              {canRunAgain && (
                <Button
                  variant="primary"
                  onClick={() => void start()}
                  className="w-full justify-center gap-1.5"
                >
                  <Sparkles size={12} />{' '}
                  {machineState === 'done' ? t('jobs.prep.runAgain') : t('jobs.prep.start')}
                </Button>
              )}
            </div>
          ) : undefined
        }
      >
        <div className="space-y-4 px-5 py-4">
          {showEmpty && (
            <EmptyState
              icon={Sparkles}
              title={
                !canRun
                  ? !resumeId
                    ? t('jobs.prep.needsResume')
                    : t('jobs.prep.needsProvider')
                  : t('jobs.prep.readyTitle')
              }
              description={t('jobs.prep.modalHint')}
              action={
                <Button
                  variant="primary"
                  disabled={!canRun}
                  onClick={() => void start()}
                  className="gap-1.5"
                >
                  <Sparkles size={12} /> {t('jobs.prep.start')}
                </Button>
              }
              className="py-8"
            />
          )}

          {showStarting && (
            <div
              role="status"
              aria-busy="true"
              className="flex items-center gap-2 text-xs text-foreground/60"
            >
              <Loader2 size={13} className="animate-spin" aria-hidden="true" />
              {t('jobs.prep.starting')}
            </div>
          )}

          {/* Visually-hidden milestone announcer — one utterance per status
              change, not per streamed token (see the effect above); also
              carries the confirm-suspended announcement (see `handleStep`). */}
          <span role="status" aria-live="polite" className="sr-only">
            {announcement}
          </span>

          {/* The run is genuinely SUSPENDED while this is pending — shown above
              the checklist so it's the first thing visible (no scrolling
              needed) and `AgentConfirm` moves focus to itself on mount. */}
          {pendingConfirm && runJobId && (
            <AgentConfirm
              jobId={runJobId}
              confirm={pendingConfirm}
              onResolved={handleConfirmResolved}
            />
          )}

          {showLog && (
            <div role="group" aria-label={t('jobs.prep.liveRegionLabel')} className="space-y-3">
              {rows.map(({ item, match, status }) => (
                <div
                  key={item.key}
                  className="rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2.5"
                >
                  <div className="flex items-center gap-2">
                    {status === 'done' && (
                      <CheckCircle2
                        size={13}
                        className="shrink-0 text-emerald-400"
                        aria-hidden="true"
                      />
                    )}
                    {status === 'active' && (
                      <Loader2
                        size={13}
                        className="shrink-0 animate-spin text-brand-soft"
                        aria-hidden="true"
                      />
                    )}
                    {status === 'interrupted' && (
                      <CircleMinus
                        size={13}
                        className="shrink-0 text-amber-400/80"
                        aria-hidden="true"
                      />
                    )}
                    {status === 'pending' && (
                      <Circle
                        size={13}
                        className="shrink-0 text-foreground/25"
                        aria-hidden="true"
                      />
                    )}
                    <span className="text-xs font-medium text-foreground/80">
                      {t(`jobs.prep.steps.${item.key}`)}
                    </span>
                    <Tag
                      color={
                        status === 'done'
                          ? 'success'
                          : status === 'active'
                            ? 'processing'
                            : status === 'interrupted'
                              ? 'warning'
                              : 'default'
                      }
                      className="ml-auto text-[9px]"
                    >
                      {t(`jobs.prep.status.${status}`)}
                    </Tag>
                  </div>
                  {/* The propose row is status-only — its text lives in the
                      dedicated proposal card below, not duplicated here. */}
                  {item.key !== 'propose' && match && (
                    <StreamingText
                      text={match.text}
                      isStreaming={status === 'active'}
                      className="mt-1.5 text-[11px]"
                    />
                  )}
                </div>
              ))}
            </div>
          )}

          {proposalStep && (
            <GlassCard
              tone="surface"
              className="space-y-2 !p-4 border-l-2 border-[var(--color-brand)]"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-semibold text-foreground/85">
                  {t('jobs.prep.proposalTitle')}
                </span>
                {result && (
                  <Tag
                    color={result.stoppedReason === 'done' ? 'success' : 'warning'}
                    className="text-[9px]"
                  >
                    {t(`jobs.prep.stopped.${stoppedSuffix}` as const, {
                      defaultValue: result.stoppedReason,
                    })}
                  </Tag>
                )}
              </div>
              <p className="text-[10px] text-foreground/45">{t('jobs.prep.proposalHint')}</p>
              <StreamingText text={proposalStep.text} className="text-[11px]" />
            </GlassCard>
          )}

          {machineState === 'cancelled' && (
            <div className="flex items-center gap-2 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2.5 text-xs text-foreground/60">
              <Square size={12} className="shrink-0 text-foreground/40" aria-hidden="true" />
              {t('jobs.prep.stopped.cancelled')}
            </div>
          )}

          {machineState === 'error' && (
            <ErrorState
              title={t('jobs.prep.runFailed')}
              description={error ?? undefined}
              className="py-8"
            />
          )}
        </div>
      </ModalShell>
    </>
  );
}
