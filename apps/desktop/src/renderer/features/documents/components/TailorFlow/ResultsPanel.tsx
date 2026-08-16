import { Check, History, RefreshCw, Settings2, X } from 'lucide-react';
import { useState } from 'react';

import type { PipelineRunSummary } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, ModalShell, Tag } from '@ajh/ui';

import { PipelineRunsList } from '@/components/generation/PipelineRunsList';
import type { QualityPipelineReview } from '@/components/generation/QualityReportPanel';
import type { GenerationMeta, LetterLayoutId, QualityReport, TemplateId } from '@/lib/generate';
import { stoppedSuffix } from '@/lib/stopped-reason';

import { GenerationOutput } from './GenerationOutput';
import { PIPELINE_STEP_KEYS } from './lib/pipeline-steps';
import type { TailorTarget } from './lib/tailor-target';

/** How a TERMINAL staged run ended — the only states this panel ever shows
 *  (a busy session renders `GeneratingPanel` instead). */
export type TailorRunState = 'done' | 'needsReview' | 'cancelled' | 'error';

const STATUS_TONE: Record<TailorRunState, 'success' | 'warning' | 'error' | 'default'> = {
  done: 'success',
  needsReview: 'warning',
  cancelled: 'default',
  error: 'error',
};

/**
 * `TailorRunState` → the `pipeline.status.*` key that actually exists.
 *
 * The two vocabularies are NOT the same: `pipeline.status` is
 * running/completed/needsReview/failed/cancelled (the Rust `PipelineRunStatus`
 * wire enum); `TailorRunState` is done/needsReview/cancelled/error (this
 * panel's own terminal states — 'done' covers cold/never-started too). A
 * bare template-literal `pipeline.status.${runState}` silently renders the
 * dotted key itself for 'done' AND 'error' — i18next has no catalog entry
 * for either. `Record<TailorRunState, …>` makes this exhaustive at compile
 * time: a new `TailorRunState` member fails to type-check here until mapped.
 * Runtime-checked against the real locale JSON in ResultsPanel.i18n.test.ts.
 */
export const PIPELINE_STATUS_KEY: Record<
  TailorRunState,
  'completed' | 'needsReview' | 'failed' | 'cancelled'
> = {
  done: 'completed',
  needsReview: 'needsReview',
  cancelled: 'cancelled',
  error: 'failed',
};

interface Props {
  target: TailorTarget;
  jobDesc: string;
  onJobDescChange: (v: string) => void;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
  jobAdSummary: {
    summary: string;
    generating: boolean;
    error: string | null;
    generate: () => void;
    language: string;
    setLanguage: (v: string) => void;
  };
  // Output / doc state from useTailorPipeline.
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  // Render-time template/ATS preference (sticky store) — drives the live preview
  // and the export. Picked on the results toolbar; never regenerates.
  templateId: TemplateId;
  atsMode: boolean;
  /** Per-export document accent (6-hex); undefined = template palette. */
  accent?: string;
  /** Per-export cover-letter layout; undefined → the backend renders classic. */
  letterLayoutId?: LetterLayoutId;
  /** Export/preview market — forwarded straight through to `GenerationOutput`'s
   *  live preview (see its own doc comment). */
  market?: string;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (v: boolean) => void;
  onAccentChange: (accent: string | undefined) => void;
  onLetterLayoutChange: (id: LetterLayoutId) => void;
  output: string;
  onEdit: (text: string) => void;
  meta: GenerationMeta | null;
  report?: QualityReport | null;
  /** Section-fix / fabrication-review extras for the ACTIVE document, from
   *  `useTailorPipeline` — undefined once no run detail exists yet. */
  pipelineReview?: QualityPipelineReview;
  /** Unresolved fabrication count across BOTH documents (not just the active
   *  tab) — see `useTailorPipeline`'s `openClaimsTotal` doc comment. Drives
   *  the needsReview banner's count and its zero-claims branch. */
  openClaims?: number;
  /** Re-run validation on the active document — clears staleness after an
   *  inline edit. Omitted hides the panel's action. */
  onRecheck?: () => void;
  rechecking?: boolean;
  copied: boolean;
  onCopy: () => void;
  exportOpen: boolean;
  setExportOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => void;
  /** How the run this document came from ended — drives the status banner. */
  runState: TailorRunState;
  /** True for a COLD redisplay (no run started/reconnected THIS session) —
   *  no interactive fix/resolve UI is wired for it, so the needsReview hint
   *  says the run needs reopening instead of implying an action available
   *  right here. */
  cold?: boolean;
  /** A start failure, or the terminal error text for `runState === 'error'`. */
  error?: string | null;
  stoppedReason?: string | null;
  /** This posting's run history (≤3, newest first) — read-only here, since
   *  this panel's document view always shows the LATEST run's text. */
  runs?: PipelineRunSummary[];
  // Actions.
  onRegenerate: () => void;
  onEditSettings: () => void;
}

/**
 * Done stage: the staged run's status (prominent for anything other than a
 * clean finish), the tailored documents (resume/cover/job-ad tabs via
 * {@link GenerationOutput}, which now also carries the section-fix /
 * fabrication-review extras), this posting's run history, and the
 * regenerate / edit-settings footer.
 */
export function ResultsPanel({
  target,
  jobDesc,
  onJobDescChange,
  hasDesc,
  fetchingDesc,
  jobUrl,
  jobAdSummary,
  activeOut,
  setActiveOut,
  templateId,
  atsMode,
  accent,
  letterLayoutId,
  market,
  onTemplateChange,
  onAtsModeChange,
  onAccentChange,
  onLetterLayoutChange,
  output,
  onEdit,
  meta,
  report,
  pipelineReview,
  openClaims = 0,
  onRecheck,
  rechecking,
  copied,
  onCopy,
  exportOpen,
  setExportOpen,
  onExport,
  runState,
  cold = false,
  error,
  stoppedReason,
  runs = [],
  onRegenerate,
  onEditSettings,
}: Props) {
  const { t } = useTranslation();
  const suffix = stoppedSuffix(stoppedReason);
  // A run can report `status: completed` (→ `runState === 'done'`) even when
  // it was truncated by the deadline mid-validate/humanize but still had a
  // document to save (`hooks.rs`'s `timed_out_with_document`) — `suffix` then
  // carries the real reason (`runTimeout`) alongside the clean-looking state.
  // Gate the banner on EITHER signal, not just `runState`, or that caveat is
  // suppressed entirely while `PipelineRunsList` shows the truth just below.
  const showBanner = runState !== 'done' || (!!suffix && suffix !== 'done');
  // "All steps completed" only when nothing cut the run short — a truncated
  // 'done' (above) genuinely didn't finish all four steps, so it gets the
  // caveat banner instead of a claim it can't back up.
  const cleanFinish = runState === 'done' && (!suffix || suffix === 'done');
  const [runsOpen, setRunsOpen] = useState(false);

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Status banner — visible without a click, unlike the quality badge
          inside GenerationOutput, so needsReview/cancelled/error never hide
          behind a small chip. Omitted on a clean finish (nothing to say). */}
      {showBanner && (
        <div className="flex shrink-0 flex-col gap-1.5 px-8 pt-4">
          <div className="flex flex-wrap items-center gap-2">
            <Tag color={STATUS_TONE[runState]} className="text-[9px]">
              {t(`pipeline.status.${PIPELINE_STATUS_KEY[runState]}`)}
            </Tag>
            {suffix && (
              <span className="text-[10px] text-foreground/45">
                {t(`pipeline.stopped.${suffix}`)}
              </span>
            )}
          </div>
          {runState === 'needsReview' && (
            <div
              role="status"
              className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2.5"
            >
              {openClaims > 0 ? (
                <>
                  <p className="text-[11px] font-medium text-amber-400">
                    {t('autopilot.apply.wizard.results.needsReviewTitle', { count: openClaims })}
                  </p>
                  <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
                    {t(
                      cold
                        ? 'autopilot.apply.wizard.results.needsReviewHintCold'
                        : 'autopilot.apply.wizard.results.needsReviewHint'
                    )}
                  </p>
                </>
              ) : (
                // The report can flag `needsReview` on a critical with NO
                // fabrication entry (Rust `slot_has_unresolvable_critical`) —
                // "0 claims need your verdict" then points at an empty list.
                // Say what's actually true and point at the quality badge
                // instead of a per-claim count that doesn't exist.
                <>
                  <p className="text-[11px] font-medium text-amber-400">
                    {t('autopilot.apply.wizard.results.needsReviewTitleEmpty')}
                  </p>
                  <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
                    {t('autopilot.apply.wizard.results.needsReviewHintEmpty')}
                  </p>
                </>
              )}
            </div>
          )}
          {runState === 'cancelled' && (
            <p className="text-[11px] leading-relaxed text-foreground/50">
              {t('autopilot.apply.wizard.results.cancelledHint')}
            </p>
          )}
          {runState === 'error' && (
            <div
              role="alert"
              className="rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2.5"
            >
              <p className="text-[11px] font-medium text-red-400">
                {t('autopilot.apply.wizard.results.failedTitle')}
              </p>
              {error && (
                <p className="mt-1 text-[10px] leading-relaxed text-foreground/60">{error}</p>
              )}
              <p className="mt-1 text-[10px] leading-relaxed text-foreground/60">
                {t('autopilot.apply.wizard.results.failedHint')}
              </p>
            </div>
          )}
        </div>
      )}

      {/* "It should mark it as done" — the owner's headline ask. The 4-step
          checklist (GeneratingPanel) unmounts the instant the run finishes,
          so it can never actually SHOW all four checked; this is the durable
          place a clean finish says so. */}
      {cleanFinish && (
        <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 px-8 pt-4 text-[10px] text-foreground/70">
          <span className="sr-only">{t('pipeline.step.allDone')}</span>
          {PIPELINE_STEP_KEYS.map((key) => (
            <span key={key} className="flex items-center gap-1">
              <Check size={10} className="text-emerald-400" aria-hidden="true" />
              {t(`pipeline.step.${key}.label`)}
            </span>
          ))}
        </div>
      )}

      {/* Height-bounded body — NOT a scroll container. GenerationOutput owns its
          own scroll boundary (below its pinned tab/action header), so the header
          stays visible while the document scrolls. Making this scroll again would
          re-break that: the whole viewer, header included, would scroll away. */}
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden px-8 py-6">
        <GenerationOutput
          target={target}
          activeOut={activeOut}
          setActiveOut={setActiveOut}
          templateId={templateId}
          atsMode={atsMode}
          accent={accent}
          letterLayoutId={letterLayoutId}
          market={market}
          onTemplateChange={onTemplateChange}
          onAtsModeChange={onAtsModeChange}
          onAccentChange={onAccentChange}
          onLetterLayoutChange={onLetterLayoutChange}
          output={output}
          onEdit={onEdit}
          editable
          meta={meta}
          report={report}
          pipeline={pipelineReview}
          onRecheck={onRecheck}
          rechecking={rechecking}
          copied={copied}
          onCopy={onCopy}
          exportOpen={exportOpen}
          setExportOpen={setExportOpen}
          onExport={onExport}
          jobDesc={jobDesc}
          onJobDescChange={onJobDescChange}
          hasDesc={hasDesc}
          fetchingDesc={fetchingDesc}
          jobUrl={jobUrl}
          jobAdSummary={jobAdSummary}
        />
      </div>

      <div className="flex shrink-0 items-center justify-between border-t border-[var(--border-clear)] px-8 py-4">
        <div className="flex items-center gap-1.5">
          <Button
            variant="ghost"
            onClick={onEditSettings}
            className="gap-1.5 text-foreground/50 hover:text-foreground/80"
          >
            <Settings2 size={13} /> {t('autopilot.apply.wizard.results.edit')}
          </Button>
          {/* Run telemetry, behind a click — mirrors the quality badge's own
              modal (QualityReportPanel/FabricationReview) instead of staying
              permanently mounted chrome on every clean finish. */}
          {runs.length > 0 && (
            <Button
              variant="ghost"
              onClick={() => setRunsOpen(true)}
              className="gap-1.5 text-foreground/50 hover:text-foreground/80"
            >
              <History size={13} /> {t('pipeline.runs.title')}
            </Button>
          )}
        </div>
        <Button variant="glass" onClick={onRegenerate} className="gap-1.5">
          <RefreshCw size={13} /> {t('autopilot.apply.wizard.results.regenerate')}
        </Button>
      </div>

      <ModalShell
        open={runsOpen}
        onClose={() => setRunsOpen(false)}
        maxWidth="max-w-xl"
        ariaLabelledby="pipeline-runs-title"
        header={
          <div className="flex items-center justify-between border-b border-white/[0.08] px-5 py-4">
            <h2 id="pipeline-runs-title" className="text-sm font-semibold text-foreground/90">
              {t('pipeline.runs.title')}
            </h2>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => setRunsOpen(false)}
              aria-label={t('common.close')}
              className="h-7 w-7 p-0"
            >
              <X size={14} />
            </Button>
          </div>
        }
      >
        <PipelineRunsList runs={runs} className="max-h-96 overflow-y-auto p-4" />
      </ModalShell>
    </div>
  );
}
