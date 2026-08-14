import { RefreshCw, Settings2 } from 'lucide-react';

import type { PipelineRunSummary } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, Tag } from '@ajh/ui';

import { PipelineRunsList } from '@/components/generation/PipelineRunsList';
import type { QualityPipelineReview } from '@/components/generation/QualityReportPanel';
import {
  type GenerationMeta,
  type LetterLayoutId,
  type QualityReport,
  type TemplateId,
  unresolvedCount,
} from '@/lib/generate';
import { stoppedSuffix } from '@/lib/stopped-reason';

import { GenerationOutput } from './GenerationOutput';
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
  onTemplateChange,
  onAtsModeChange,
  onAccentChange,
  onLetterLayoutChange,
  output,
  onEdit,
  meta,
  report,
  pipelineReview,
  onRecheck,
  rechecking,
  copied,
  onCopy,
  exportOpen,
  setExportOpen,
  onExport,
  runState,
  error,
  stoppedReason,
  runs = [],
  onRegenerate,
  onEditSettings,
}: Props) {
  const { t } = useTranslation();
  const suffix = stoppedSuffix(stoppedReason);
  const openClaims = pipelineReview ? unresolvedCount(pipelineReview.fabrications, output) : 0;

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Status banner — visible without a click, unlike the quality badge
          inside GenerationOutput, so needsReview/cancelled/error never hide
          behind a small chip. Omitted on a clean finish (nothing to say). */}
      {runState !== 'done' && (
        <div className="flex shrink-0 flex-col gap-1.5 px-8 pt-4">
          <div className="flex flex-wrap items-center gap-2">
            <Tag color={STATUS_TONE[runState]} className="text-[9px]">
              {t(`pipeline.status.${runState === 'error' ? 'failed' : runState}`)}
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
              className="rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2.5"
            >
              <p className="text-[11px] font-medium text-amber-400">
                {t('autopilot.apply.wizard.results.needsReviewTitle', { count: openClaims })}
              </p>
              <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
                {t('autopilot.apply.wizard.results.needsReviewHint')}
              </p>
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
              className="rounded-lg border border-red-500/20 bg-red-500/5 px-3 py-2.5"
            >
              <p className="text-[11px] font-medium text-red-400">
                {t('autopilot.apply.wizard.results.failedTitle')}
              </p>
              {error && (
                <p className="mt-1 text-[10px] leading-relaxed text-foreground/60">{error}</p>
              )}
              <p className="mt-1 text-[10px] leading-relaxed text-foreground/45">
                {t('autopilot.apply.wizard.results.failedHint')}
              </p>
            </div>
          )}
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

      {runs.length > 0 && (
        <div className="max-h-40 shrink-0 overflow-y-auto border-t border-[var(--border-clear)] px-8 py-3">
          <PipelineRunsList runs={runs} />
        </div>
      )}

      <div className="flex shrink-0 items-center justify-between border-t border-[var(--border-clear)] px-8 py-4">
        <Button
          variant="ghost"
          onClick={onEditSettings}
          className="gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <Settings2 size={13} /> {t('autopilot.apply.wizard.results.edit')}
        </Button>
        <Button variant="glass" onClick={onRegenerate} className="gap-1.5">
          <RefreshCw size={13} /> {t('autopilot.apply.wizard.results.regenerate')}
        </Button>
      </div>
    </div>
  );
}
