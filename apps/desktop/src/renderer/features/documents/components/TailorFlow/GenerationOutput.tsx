import { Check, Copy, Download } from 'lucide-react';
import { useMemo, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, type TabItem, Tabs } from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import { type ExportFormat, ExportPicker } from '@/components/generation/ExportPicker';
import { HandEditNudge } from '@/components/generation/HandEditNudge';
import { PdfPreview } from '@/components/generation/PdfPreview';
import {
  QualityBadge,
  type QualityPipelineReview,
} from '@/components/generation/QualityReportPanel';
import type { GenerationMeta, LetterLayoutId, QualityReport, TemplateId } from '@/lib/generate';

import { OutputOptionStrips } from './GenerationOutput/OutputOptionStrips';
import { useCommittedPreview } from './GenerationOutput/useCommittedPreview';
import { useScoreSnapshot } from './GenerationOutput/useScoreSnapshot';
import { GenerationScoreStrip } from './GenerationScoreStrip';
import { JobAdView } from './JobAdView';
import type { TailorTarget } from './lib/tailor-target';

interface Props {
  target: TailorTarget;
  /** Whether the posting has a saved tailored résumé to show at all. Only ever
   *  `false` for a cover-only run on a posting that has never produced one —
   *  a `resume`/`both` run always has (or is producing) its own. */
  hasResume: boolean;
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  // Render-time template/ATS (sticky store) — drives BOTH the preview here and the
  // export in useTailorPipeline. The toolbar picker mutates them; no regeneration.
  templateId: TemplateId;
  atsMode: boolean;
  /** Per-export document accent (6-hex); undefined = template palette. */
  accent?: string;
  /** Per-export cover-letter layout; undefined → the backend renders classic. */
  letterLayoutId?: LetterLayoutId;
  /** Export/preview market (from `useTailorPipeline`'s `resolveMarket`) — mirrors
   *  the real export so the live preview's letter conventions (salutation,
   *  sign-off) match the downloaded document instead of silently falling back
   *  to "intl" (see `PdfPreview`'s `locale` prop). */
  market?: string;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (v: boolean) => void;
  onAccentChange: (accent: string | undefined) => void;
  onLetterLayoutChange: (id: LetterLayoutId) => void;
  output: string;
  onEdit: (text: string) => void;
  editable: boolean;
  meta: GenerationMeta | null;
  report?: QualityReport | null;
  /**
   * Staged-run extras for the ACTIVE document's badge/panel (section Fix,
   * per-bullet fabrication review) — see `QualityPipelineReview`. Absent for
   * a fast-path report; the badge then renders exactly as it always did.
   */
  pipeline?: QualityPipelineReview;
  /** Re-run validation on the active document — this is the only surface with
   *  inline editing, so it is also the only one that can go stale mid-session. */
  onRecheck?: () => void;
  rechecking?: boolean;
  copied: boolean;
  onCopy: () => void;
  exportOpen: boolean;
  setExportOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => void;
  jobDesc: string;
  onJobDescChange: (v: string) => void;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
  /** Saved résumé backing this generation — threaded to the job-ad tab's Score view. */
  resumeId?: string;
  jobAdSummary: {
    summary: string;
    generating: boolean;
    error: string | null;
    generate: () => void;
    language: string;
    setLanguage: (v: string) => void;
  };
}

export function GenerationOutput({
  target,
  hasResume,
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
  editable,
  meta,
  report,
  pipeline,
  onRecheck,
  rechecking,
  copied,
  onCopy,
  exportOpen,
  setExportOpen,
  onExport,
  jobDesc,
  onJobDescChange,
  hasDesc,
  fetchingDesc,
  jobUrl,
  resumeId,
  jobAdSummary,
}: Props) {
  const { t } = useTranslation();
  const [view, setView] = useState<'doc' | 'jobAd'>('doc');
  // Highlighted format in the export picker. The picker is immediate (a click
  // downloads), so this only tracks the visual selection between opens.
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');

  const scoreSnapshot = useScoreSnapshot(jobDesc, report?.generatedAt);
  const { committed, pending, handleEdit, handleBlur } = useCommittedPreview(
    activeOut,
    output,
    onEdit
  );
  const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';

  // Does this panel show a résumé tab at all? ONE definition, read by both
  // the tab list below and the ATS-flag release in `OutputOptionStrips` — a
  // `resume`/`both` run always has one; a cover-only run does only when the
  // posting already carries a saved tailored résumé from an earlier run. A
  // cover-only run still has a templateId (it supplies the letter's palette),
  // so a design-tier id must not keep the shared ATS flag alive on a résumé that
  // is not there — deliberately the SAME predicate as the tab list rather than
  // `target !== 'cover'`, since a saved résumé stays an exportable tab and
  // releasing the flag out from under a document the user can still see and
  // export is the failure this shares a definition to prevent.
  const hasResumeTab = target !== 'cover' || hasResume;

  // ARIA tabs contract: each tab owns a stable id and controls a panel id; the
  // single content region below is the active tab's panel (doc tabs share one
  // region, the Job-ad tab swaps in its own). Derive the active pair so the
  // panel can label itself back to whichever tab is selected.
  const activeTabKey = view === 'jobAd' ? 'jobad' : activeOut;
  const activeTabId = `tailor-tab-${activeTabKey}`;
  const activePanelId = `tailor-panel-${activeTabKey}`;

  type TabKey = 'resume' | 'cover' | 'jobad';
  const tabItems = useMemo<readonly TabItem<TabKey>[]>(() => {
    // Off `target`, never `activeOut`: deriving the tab LIST from which tab
    // happens to be SELECTED is what made a cover-only run render exactly one
    // tab, labelled "Resume", showing the résumé — the reported bug. The
    // résumé tab is now structurally absent for a cover-only run rather than
    // merely unselected, except when the posting already has a saved tailored
    // résumé from an earlier run, which stays viewable and exportable
    // (review-inert — see `useTailorPipeline`'s `reviewableOut`).
    const docKeys: ('resume' | 'cover')[] =
      target === 'both'
        ? ['resume', 'cover']
        : target === 'cover'
          ? hasResumeTab
            ? ['cover', 'resume']
            : ['cover']
          : ['resume'];
    const items: TabItem<TabKey>[] = docKeys.map((o) => ({
      value: o,
      label:
        o === 'resume' ? t('autopilot.apply.target.resume') : t('autopilot.apply.target.cover'),
      id: `tailor-tab-${o}`,
      ariaControls: `tailor-panel-${o}`,
    }));
    items.push({
      value: 'jobad',
      label: t('autopilot.apply.tabs.jobAd'),
      id: 'tailor-tab-jobad',
      ariaControls: 'tailor-panel-jobad',
    });
    return items;
  }, [target, hasResumeTab, t]);

  const handleTabChange = (key: TabKey) => {
    if (key === 'jobad') {
      setView('jobAd');
    } else {
      setView('doc');
      setActiveOut(key);
    }
  };

  // Three-part shape (mirrors the AI-Generate viewer + ModalShell, docs/PATTERNS.md §13):
  // the root is HEIGHT-BOUNDED (`min-h-0 flex-1` inside the caller's `h-full` column)
  // instead of growing past it, so the tab/action header stays put and the scrollport
  // below it does the scrolling. Never give the root an intrinsic min-height — that
  // pushes the scroll boundary back up to the caller and the header scrolls away with
  // the document (the bug this fixes). `overflow-hidden` keeps the rounded border a
  // real clip; every popover inside is portalled/fixed, so nothing is lost to it.
  // The height chain above this component is load-bearing too — see TailorFlow's
  // `min-h-0 flex-1` stage body (asserted in TailorFlow.test.tsx).
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-foreground/[0.06] bg-foreground/[0.02]">
      <div className="shrink-0 flex items-center justify-between border-b border-foreground/[0.06] px-3 py-2">
        <Tabs
          ariaLabel={t('autopilot.apply.tabs.outputTabs')}
          items={tabItems}
          value={activeTabKey}
          onChange={handleTabChange}
          size="sm"
          className="border-none"
        />
        <div className="flex items-center gap-1">
          {view === 'doc' && (
            <QualityBadge
              report={report}
              docKind={activeOut === 'resume' ? 'resume' : 'coverLetter'}
              currentText={output}
              pipeline={pipeline}
              onRecheck={onRecheck}
              rechecking={rechecking}
              // The editor's own change handler, so a "Remove" from the review
              // is an ordinary edit and follows the same commit/save path.
              // Withheld while the document is locked (`editable === false`) —
              // the review then shows "marked for removal" rather than
              // pretending the line is gone.
              onDocumentTextChange={editable ? handleEdit : undefined}
              className="mr-1"
            />
          )}
          <Button
            onClick={() => void onCopy()}
            disabled={!output || view === 'jobAd'}
            className="flex h-auto items-center gap-1.5 rounded border border-transparent bg-transparent px-2 py-1 text-[10px] text-foreground/45 transition-colors hover:bg-foreground/[0.04] hover:text-foreground/70 disabled:opacity-40 disabled:pointer-events-none"
          >
            {copied ? <Check size={11} /> : <Copy size={11} />}
            {copied ? t('autopilot.apply.copied') : t('autopilot.apply.copy')}
          </Button>
          <Button
            onClick={() => setExportOpen(true)}
            disabled={!output || view === 'jobAd'}
            className="flex h-auto items-center gap-1.5 rounded border border-transparent bg-transparent px-2 py-1 text-[10px] text-brand-soft transition-colors hover:bg-brand/10 hover:text-brand-soft/90 disabled:opacity-40 disabled:pointer-events-none"
          >
            <Download size={11} />
            {t('aiGenerate.export')}
          </Button>
          {/* Format picker — now the shared, focus-trapped ModalShell-based
              ExportPicker (immediate mode): the chosen template/ATS live in the
              toolbar strip below, so picking a format downloads it right away. */}
          <ExportPicker
            open={exportOpen}
            onClose={() => setExportOpen(false)}
            format={exportFormat}
            onFormatChange={setExportFormat}
            onExport={(fmt) => void onExport(fmt)}
            zIndex={700}
          />
        </div>
      </div>
      {/* Hand-edit nudge — once per generation: keyed by the report's timestamp
          so a NEW generation remounts (and re-shows) it; ordinary re-renders
          (edits, tab switches) leave a dismissal in place. Pinned like the
          toolbar above it, not part of the scrollport below. */}
      {view === 'doc' && report && <HandEditNudge key={report.generatedAt} className="mx-3 mt-2" />}
      {/* Score strip — résumé only (a cover letter isn't scored against
          keyword coverage). Surfaces the score users otherwise only find two
          clicks away, behind the opt-in Job ad → Score sub-tab. Pinned like
          the nudge above it, so it's visible without scrolling the preview. */}
      {view === 'doc' && activeOut === 'resume' && (
        <GenerationScoreStrip resumeId={resumeId} jobDesc={scoreSnapshot} className="mx-3 mt-2" />
      )}
      {/* The scrollport. ONLY the tab/action bar above pins — the option strips
          scroll WITH the document: pinning them too costs more permanent chrome
          than a small window can spare, collapsing the document to nothing and
          clipping the last strip out of reach. They are occasional controls; the
          document is the content.
          The document region below carries a `min-h-[20rem]` FLOOR — that is what
          makes this a real scrollport. Without it every child is `flex-1`/`h-full`,
          content always fits exactly and `overflow-y-auto` can never engage.
          The pinned header is a flex SIBLING of this box (not sticky chrome
          overlaying it), so it cannot obscure a focused element inside
          (WCAG 2.4.11) and needs no scroll-margin; `tabIndex={0}` keeps the
          scrollport keyboard-scrollable. */}
      <div
        role="tabpanel"
        id={activePanelId}
        aria-labelledby={activeTabId}
        tabIndex={0}
        className="flex min-h-0 flex-1 flex-col overflow-y-auto"
      >
        {view === 'doc' && (
          <OutputOptionStrips
            target={target}
            resumeInRun={hasResumeTab}
            activeOut={activeOut}
            docType={docType}
            meta={meta}
            templateId={templateId}
            atsMode={atsMode}
            accent={accent}
            letterLayoutId={letterLayoutId}
            onTemplateChange={onTemplateChange}
            onAtsModeChange={onAtsModeChange}
            onAccentChange={onAccentChange}
            onLetterLayoutChange={onLetterLayoutChange}
          />
        )}
        {/* Document region — grows to fill the scrollport, but never shrinks below
            the floor, so a short window scrolls instead of collapsing the document
            to a few pixels. */}
        <div
          data-testid={TEST_IDS.documents.documentRegion}
          className="flex min-h-[20rem] flex-1 flex-col px-3 py-2"
        >
          {view === 'jobAd' ? (
            <JobAdView
              jobDesc={jobDesc}
              onJobDescChange={onJobDescChange}
              summary={jobAdSummary.summary}
              generating={jobAdSummary.generating}
              error={jobAdSummary.error}
              onGenerateSummary={jobAdSummary.generate}
              language={jobAdSummary.language}
              onLanguageChange={jobAdSummary.setLanguage}
              hasDesc={hasDesc}
              fetchingDesc={fetchingDesc}
              jobUrl={jobUrl}
              resumeId={resumeId}
            />
          ) : (
            <EditableOutput
              value={output}
              onChange={handleEdit}
              onBlur={handleBlur}
              isPending={pending}
              disabled={!editable}
              docType={docType}
              meta={meta}
              className="flex h-full flex-col overflow-hidden"
              textAreaClassName="h-full w-full bg-transparent text-[11px] leading-relaxed text-foreground/75 placeholder:text-foreground/20"
              previewSlot={
                <PdfPreview
                  text={committed[activeOut]}
                  docType={docType}
                  meta={meta}
                  templateId={templateId}
                  atsMode={atsMode}
                  accent={accent}
                  letterLayoutId={letterLayoutId}
                  locale={market}
                  paused={!editable}
                  className="h-full w-full"
                />
              }
            />
          )}
        </div>
      </div>
    </div>
  );
}
