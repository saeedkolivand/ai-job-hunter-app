import {
  CheckCircle2,
  Circle,
  CircleDashed,
  CircleMinus,
  Loader2,
  Sparkles,
  Square,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { AgentStepEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, StreamingText, Tag } from '@ajh/ui';

import { AgentConfirm } from '@/features/jobs/components/AgentConfirm';
import type { AgentRunSession } from '@/features/jobs/hooks/useAgentRunSession';
import { stoppedSuffix } from '@/lib/stopped-reason';

export type ImproveRowKey =
  'report' | 'check' | 'trim' | 'rewrite' | 'evidence' | 'save' | 'summary';

export type ImproveRowStatus = 'pending' | 'active' | 'done' | 'interrupted' | 'skipped';

interface ImproveRowSpec {
  key: ImproveRowKey;
  /** Does this step belong to this row? A PREDICATE rather than a tool name
   *  because two rows are not tool turns at all: the gated save is announced as
   *  a `confirm_request` (whose `tools[]` is empty — the tool is in `confirm`),
   *  and the closing summary is the terminal `proposal` step. */
  matches: (step: AgentStepEvent) => boolean;
  /** The prompt rations AT MOST ONE of these per run, so a row that always
   *  rendered would sit at "Pending" through a healthy run and read as a step
   *  that never happened. Shown only once the run actually takes it. */
  optional?: boolean;
}

/**
 * The `improve_resume` flow's steps, in the order `IMPROVE_RESUME_SYSTEM`
 * drives them (report → validate → optional trim/rewrite → evidence → the
 * re-check → the gated save → summary).
 *
 * `validate_resume` appears TWICE in a run (the post-fix re-check) and matches
 * the single `check` row both times — the row re-activates rather than a second
 * one appearing, which is the honest reading: it is the same check, run again.
 */
const IMPROVE_CHECKLIST: readonly ImproveRowSpec[] = [
  { key: 'report', matches: (s) => s.tools.includes('get_quality_report') },
  { key: 'check', matches: (s) => s.tools.includes('validate_resume') },
  { key: 'trim', matches: (s) => s.tools.includes('get_trim_suggestions'), optional: true },
  { key: 'rewrite', matches: (s) => s.tools.includes('run_quality_pipeline'), optional: true },
  { key: 'evidence', matches: (s) => s.tools.includes('search_candidate_evidence') },
  {
    key: 'save',
    // The turn that ASKS (a `turn` step naming the tool) and the suspend it
    // produces (`confirm_request`, `tools` empty) are the same row.
    matches: (s) =>
      s.tools.includes('save_resume') ||
      (s.kind === 'confirm_request' && s.confirm?.tool === 'save_resume'),
  },
  { key: 'summary', matches: (s) => s.kind === 'proposal' },
];

export interface ImproveRow {
  key: ImproveRowKey;
  /** The LAST step matching this row, or `undefined` while it is pending. */
  match: AgentStepEvent | undefined;
  status: ImproveRowStatus;
}

/**
 * Rows for the run so far: every mandatory step plus the optional ones the run
 * actually took.
 *
 * Three statuses depend on the run being OVER, not on it having failed:
 *
 * - `active` only while the row is the LATEST step of a live run;
 * - `interrupted` for that same latest step once the run ended early (stopped,
 *   failed, or "completed" at a ceiling) — it was in flight and never finished;
 * - `skipped` for anything still unmatched once the run is over. "Pending" on a
 *   run that has ended claims work that is still coming; a review stopped at
 *   its tool-call limit never reaches its last steps, and the user has to be
 *   able to see which checks did not run before trusting the résumé.
 */
export function buildImproveRows(
  steps: AgentStepEvent[],
  { busy, interrupted }: { busy: boolean; interrupted: boolean }
): ImproveRow[] {
  const last = steps[steps.length - 1];
  const rows: ImproveRow[] = [];
  for (const spec of IMPROVE_CHECKLIST) {
    let match: AgentStepEvent | undefined;
    for (let i = steps.length - 1; i >= 0; i--) {
      const step = steps[i];
      if (step && spec.matches(step)) {
        match = step;
        break;
      }
    }
    if (!match && spec.optional) continue;
    const isLatest = !!match && last === match;
    rows.push({
      key: spec.key,
      match,
      status: !match
        ? busy
          ? 'pending'
          : 'skipped'
        : isLatest && busy
          ? 'active'
          : isLatest && interrupted
            ? 'interrupted'
            : 'done',
    });
  }
  return rows;
}

/** The card's own headline status — what the run is doing right now. */
function headlineState(
  session: AgentRunSession
): 'waiting' | 'running' | 'done' | 'cancelled' | 'failed' {
  if (session.state === 'confirming') return 'waiting';
  if (session.busy) return 'running';
  if (session.state === 'error') return 'failed';
  if (session.state === 'cancelled') return 'cancelled';
  return 'done';
}

export interface ImproveResumeRunProps {
  session: AgentRunSession;
  /**
   * Called after `dismiss`, once this card is about to unmount — the host parks
   * focus on a node it owns (its own trigger). Without it focus falls to
   * `<body>`, behind the modal's portal, where the focus trap cannot recover
   * it: the next Tab starts from the top of the document.
   */
  onDismissed?: () => void;
}

/**
 * The `improve_resume` flow's progress, rendered where the run was started
 * (`TailoredResumePanel`'s modal body) rather than in a second dialog: the run
 * SUSPENDS on its gated save, and a confirm card behind another focus trap is a
 * decision the user cannot reach.
 *
 * A compact sibling of `PrepApplicationPanel`'s checklist, not a reuse of it —
 * that panel's rows are the prep sequence (research/match/draft/questions),
 * which this flow never runs. What is shared is the shape: last-matching-step
 * per row, one live-region utterance per status change (never per streamed
 * token), and `AgentConfirm` mounted above the log so the suspended decision is
 * the first thing visible.
 */
export function ImproveResumeRun({ session, onDismissed }: ImproveResumeRunProps) {
  const { t } = useTranslation();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const prevStatusRef = useRef<Partial<Record<ImproveRowKey, ImproveRowStatus>>>({});
  const announcedTerminalRef = useRef<string | null>(null);
  const [announcement, setAnnouncement] = useState('');

  // This card mounts exactly when the user starts a run — send focus to it, or
  // a keyboard user is left on a button whose effect is somewhere off-screen.
  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  const state = headlineState(session);
  /**
   * A run that "completed" at a ceiling did not finish. `job.completed` fires
   * for `MaxSteps`/`MaxTokens`/`MaxToolCalls`/`Truncated` too, so an unfinished
   * run must not wear the finished badge, its rows must not claim the checks it
   * never reached, and the reason has to be said in a sentence rather than
   * crammed into a 9px badge. An unmapped variant lands on the neutral
   * "Stopped" copy, never on "Completed". The reason vocabulary is the
   * pipeline's, through the shared enum map — the Rust enum is shared, so the
   * mapping is.
   */
  const earlyStop = (() => {
    if (state !== 'done') return null;
    const suffix = stoppedSuffix(session.stoppedReason);
    return suffix && suffix !== 'done' ? suffix : null;
  })();

  const rows = buildImproveRows(session.steps, {
    busy: session.busy,
    // A ceiling stop is an interruption even though the job "completed".
    interrupted: session.interrupted || !!earlyStop,
  });

  // One utterance per status CHANGE (not per streamed character): the narration
  // itself stays outside any live region. `rows` is rebuilt every render, so
  // this effect re-runs every render too — the ref guard makes that a no-op
  // except on a real change.
  useEffect(() => {
    for (const { key, status } of rows) {
      if (prevStatusRef.current[key] === status) continue;
      prevStatusRef.current[key] = status;
      if (status === 'pending') continue;
      setAnnouncement(
        `${t(`jobs.tailored.improve.steps.${key}`)} — ${t(`jobs.tailored.improve.status.${status}`)}`
      );
    }
  }, [rows, t]);

  // The suspend is its own announcement: the run is blocked on the user.
  useEffect(() => {
    if (session.pendingConfirm) setAnnouncement(t('jobs.tailored.improve.confirmAnnouncement'));
  }, [session.pendingConfirm, t]);

  // …and so is the outcome. Without this the run just stops narrating: a screen
  // reader hears the last step's status and never learns the run ended, or how.
  // Guarded by the announced state so a re-render cannot repeat it.
  useEffect(() => {
    if (session.busy) {
      announcedTerminalRef.current = null;
      return;
    }
    const outcome = earlyStop ? `stopped:${earlyStop}` : state;
    if (announcedTerminalRef.current === outcome) return;
    announcedTerminalRef.current = outcome;
    setAnnouncement(
      earlyStop ? t(`pipeline.stopped.${earlyStop}`) : t(`jobs.tailored.improve.state.${state}`)
    );
  }, [session.busy, state, earlyStop, t]);

  const proposal = session.steps.find((s) => s.kind === 'proposal');
  const starting = session.busy && session.steps.length === 0;

  return (
    <GlassCard
      tone="surface"
      role="region"
      aria-labelledby="improve-resume-run-heading"
      className="space-y-3 !p-4 border-l-2 border-[var(--color-brand)]"
    >
      <div className="flex items-center gap-2">
        <Sparkles size={13} className="shrink-0 text-brand-soft" aria-hidden="true" />
        <h3
          ref={headingRef}
          id="improve-resume-run-heading"
          tabIndex={-1}
          className="text-xs font-semibold text-foreground/85 outline-none"
        >
          {t('jobs.tailored.improve.heading')}
        </h3>
        {/* A badge is a LABEL, not a sentence: the specific reason lives in the
            paragraph below, where it can be read at a normal size and does not
            force the header to wrap or truncate. */}
        <Tag
          color={
            earlyStop
              ? 'warning'
              : state === 'done'
                ? 'success'
                : state === 'failed'
                  ? 'error'
                  : state === 'waiting'
                    ? 'warning' // blocked on the user reads differently from running
                    : state === 'running'
                      ? 'processing'
                      : 'default'
          }
          className="ml-auto text-[9px]"
        >
          {earlyStop
            ? t('jobs.tailored.improve.state.stopped')
            : t(`jobs.tailored.improve.state.${state}`)}
        </Tag>
      </div>

      {/* Present tense — only true while the run is going. */}
      {session.busy && (
        <div className="space-y-1 text-[11px] leading-relaxed text-foreground/60">
          <p>{t('jobs.tailored.improve.hint')}</p>
          <p>{t('jobs.tailored.improve.optionalNote')}</p>
        </div>
      )}

      {/* Visually-hidden milestone announcer — see the effects above. */}
      <span role="status" aria-live="polite" className="sr-only">
        {announcement}
      </span>

      {/* The run is genuinely SUSPENDED while this is pending, so it sits above
          the log; `AgentConfirm` moves focus to itself on mount. */}
      {session.pendingConfirm && session.runJobId && (
        <AgentConfirm
          jobId={session.runJobId}
          confirm={session.pendingConfirm}
          onResolved={(event) => {
            // The card (and the button just clicked) is about to unmount —
            // re-park focus on this heading, which is always here.
            headingRef.current?.focus();
            session.resolveConfirm(event);
          }}
        />
      )}

      {starting && (
        <div
          role="status"
          aria-busy="true"
          className="flex items-center gap-2 text-[11px] text-foreground/60"
        >
          <Loader2 size={13} className="animate-spin" aria-hidden="true" />
          {t('jobs.tailored.improve.starting')}
        </div>
      )}

      {/* Not while starting: five Pending rows under a "starting" spinner say
          the same nothing twice. */}
      {!starting && rows.length > 0 && (
        <div
          role="group"
          aria-label={t('jobs.tailored.improve.liveRegionLabel')}
          className="space-y-2"
        >
          {rows.map(({ key, match, status }) => (
            <div
              key={key}
              className="rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2"
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
                  <CircleMinus size={13} className="shrink-0 text-amber-400" aria-hidden="true" />
                )}
                {status === 'skipped' && (
                  <CircleDashed
                    size={13}
                    className="shrink-0 text-foreground/40"
                    aria-hidden="true"
                  />
                )}
                {status === 'pending' && (
                  <Circle size={13} className="shrink-0 text-foreground/40" aria-hidden="true" />
                )}
                <span className="text-[11px] font-medium text-foreground/80">
                  {t(`jobs.tailored.improve.steps.${key}`)}
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
                  className="ml-auto shrink-0 text-[9px]"
                >
                  {t(`jobs.tailored.improve.status.${status}`)}
                </Tag>
              </div>
              {/* The summary row is status-only — its text is the card below,
                  not rendered twice. */}
              {key !== 'summary' && match && match.text && (
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

      {proposal && (
        <div className="rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2.5">
          <p className="text-[11px] font-semibold text-foreground/85">
            {t('jobs.tailored.improve.summaryTitle')}
          </p>
          <StreamingText text={proposal.text} className="mt-1 text-[11px]" />
        </div>
      )}

      {/* "The review is finished" is only true of a run that reached its end. */}
      {session.state === 'done' && !earlyStop && (
        <p className="text-[11px] leading-relaxed text-foreground/60">
          {t('jobs.tailored.improve.doneHint')}
        </p>
      )}

      {/* The ceiling stop's own paragraph: the specific reason, then what it
          means for the résumé — a run with unrun checks is not a verdict. */}
      {earlyStop && (
        <p className="text-[11px] leading-relaxed text-foreground/60">
          {t('jobs.tailored.improve.stoppedHint', { reason: t(`pipeline.stopped.${earlyStop}`) })}
        </p>
      )}

      {session.state === 'cancelled' && (
        <p className="text-[11px] leading-relaxed text-foreground/60">
          {t('jobs.tailored.improve.cancelledHint')}
        </p>
      )}

      {session.state === 'error' && (
        <div role="alert" className="rounded-lg border border-red-500/20 bg-red-500/5 px-3 py-2.5">
          <p className="text-[11px] font-medium text-red-400">
            {t('jobs.tailored.improve.failedTitle')}
          </p>
          {session.error && (
            <p className="mt-1 text-[10px] leading-relaxed text-foreground/60">{session.error}</p>
          )}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {session.busy && session.runJobId && (
          <Button
            variant="glass"
            // Both actions remove the control that was just clicked (Stop
            // disables itself, Dismiss unmounts the whole card), and focus left
            // on a removed node falls to `<body>` BEHIND the modal portal,
            // where the trap cannot recover it. Park it first — the same move
            // the confirm resolution already makes.
            onClick={() => {
              headingRef.current?.focus();
              session.stop();
            }}
            disabled={session.stopRequested}
            title={t('jobs.tailored.improve.stopHint')}
            className="gap-1.5 text-foreground/60"
          >
            <Square size={11} aria-hidden="true" />{' '}
            {session.stopRequested
              ? t('jobs.tailored.improve.stopping')
              : t('jobs.tailored.improve.stop')}
          </Button>
        )}
        {!session.busy && (
          <Button
            variant="ghost"
            onClick={() => {
              // This card is about to unmount, so the target has to be a node
              // the HOST owns (see `onDismissed`).
              session.dismiss();
              onDismissed?.();
            }}
            className="gap-1.5"
          >
            {t('jobs.tailored.improve.dismiss')}
          </Button>
        )}
      </div>
    </GlassCard>
  );
}
