import { Check, Copy, Download, FileText, LayoutTemplate } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, type TabItem, Tabs } from '@ajh/ui';

import { AccentPicker } from '@/components/generation/AccentPicker';
import { AtsModeToggle } from '@/components/generation/AtsModeToggle';
import { EditableOutput } from '@/components/generation/EditableOutput';
import { type ExportFormat, ExportPicker } from '@/components/generation/ExportPicker';
import { HandEditNudge } from '@/components/generation/HandEditNudge';
import { LetterLayoutPicker } from '@/components/generation/LetterLayoutPicker';
import { PdfPreview } from '@/components/generation/PdfPreview';
import {
  QualityBadge,
  type QualityPipelineReview,
} from '@/components/generation/QualityReportPanel';
import { useDebouncedCommit } from '@/hooks/use-debounced-commit';
import {
  atsModeHintKey,
  buildFilename,
  type GenerationMeta,
  isDecoratedLetterLayout,
  isDesignTier,
  type LetterLayoutId,
  type QualityReport,
  shouldClearAtsMode,
  TEMPLATE_IDS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';

import { JobAdView } from './JobAdView';
import type { TailorTarget } from './lib/tailor-target';

interface Props {
  target: TailorTarget;
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
  jobAdSummary,
}: Props) {
  const { t } = useTranslation();
  const [view, setView] = useState<'doc' | 'jobAd'>('doc');
  // Highlighted format in the export picker. The picker is immediate (a click
  // downloads), so this only tracks the visual selection between opens.
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');

  // Committed text per doc — what PdfPreview renders. Local edits auto-commit
  // after ~700 ms via useDebouncedCommit; generation/regeneration commits immediately.
  const [committed, setCommitted] = useState<Record<'resume' | 'cover', string>>({
    resume: activeOut === 'resume' ? output : '',
    cover: activeOut === 'cover' ? output : '',
  });
  const [pending, setPending] = useState(false);

  const lastEditRef = useRef<Record<'resume' | 'cover', string | null>>({
    resume: null,
    cover: null,
  });

  const commitToDoc = useCallback((out: 'resume' | 'cover', text: string) => {
    setCommitted((c) => ({ ...c, [out]: text }));
    setPending(false);
  }, []);

  const { scheduleCommit, flush, cancel } = useDebouncedCommit<'resume' | 'cover'>(commitToDoc);

  // Flush on doc/tab switch so a pending edit commits to ITS OWN doc before the
  // view changes. flush() uses the (out, value) pair captured at scheduleCommit
  // time — never the current activeOut — so the edit always lands in the right doc.
  const prevActiveOutRef = useRef(activeOut);
  useEffect(() => {
    if (prevActiveOutRef.current !== activeOut) {
      flush();
      prevActiveOutRef.current = activeOut;
    }
  }, [activeOut, flush]);

  // Cancel on unmount.
  useEffect(() => cancel, [cancel]);

  // Refresh committed when `output` changes for a reason OTHER than a local edit
  // (generation, regenerate, or tab switch). A local edit sets lastEditRef so the
  // debounce handles it instead.
  useEffect(() => {
    if (output !== lastEditRef.current[activeOut]) {
      setCommitted((c) => ({ ...c, [activeOut]: output }));
      setPending(false);
      lastEditRef.current[activeOut] = null;
    }
  }, [output, activeOut]);

  const handleEdit = useCallback(
    (value: string) => {
      lastEditRef.current[activeOut] = value;
      setPending(true);
      // Capture (activeOut, value) pair now — tab switches can't misroute the commit.
      scheduleCommit(activeOut, value);
      onEdit(value);
    },
    [activeOut, scheduleCommit, onEdit]
  );

  const handleBlur = useCallback(() => {
    // flush() commits the (out, value) pair captured at scheduleCommit time —
    // uses the typed value, never the prop, and always routes to the correct doc.
    flush();
  }, [flush]);
  const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';

  // Does the one ATS-safe flag still act on this export's cover letter? True
  // only for a run that produces a letter whose layout carries a decoration
  // (band / rail / monogram tile) the renderer drops under `data.opts.ats`.
  const letterAtsApplies = target !== 'resume' && isDecoratedLetterLayout(letterLayoutId);

  // …and is there a résumé in this export at all? A cover-only run still has a
  // templateId (it supplies the letter's palette) but renders no résumé from it,
  // so a design-tier id must not keep the shared flag alive on its behalf.
  const resumeInRun = target !== 'cover';

  // …and does it act on the document currently on screen? That is what decides
  // whether the toolbar shows the switch: the résumé tab asks about the template,
  // the cover tab about the letter layout (the tab itself proves a letter exists).
  const showAtsToggle =
    activeOut === 'cover' ? isDecoratedLetterLayout(letterLayoutId) : isDesignTier(templateId);

  // Template picker (mirrors GenerateWizard.handleTemplateChange): selecting an
  // ATS-tier template forces ATS off, since ATS-safe mode only applies to
  // design-tier layouts (two-column OR photo, incl. Lebenslauf) — unless a
  // decorated cover letter is still reading the flag, which is the one case
  // where clearing it would strand the letter's decoration with no off switch.
  // One template id drives BOTH docs' preview + export.
  const templateOptions = TEMPLATE_IDS.map((id) => ({ value: id, label: TEMPLATES[id].name }));
  const handleTemplateChange = (value: string) => {
    const id = value as TemplateId;
    onTemplateChange(id);
    if (shouldClearAtsMode(id, letterAtsApplies, resumeInRun)) onAtsModeChange(false);
  };

  // Same guard, other input: switching AWAY from a decorated layout has to
  // release the shared flag too, or the next decorated layout comes back
  // silently pre-ATS'd (user picks Monogram, exports a letter with no monogram).
  // Still keeps the flag when a design-tier résumé template is reading it — and
  // only when that résumé is actually part of the run.
  const handleLetterLayoutChange = (id: LetterLayoutId) => {
    onLetterLayoutChange(id);
    if (shouldClearAtsMode(templateId, isDecoratedLetterLayout(id), resumeInRun))
      onAtsModeChange(false);
  };

  // ARIA tabs contract: each tab owns a stable id and controls a panel id; the
  // single content region below is the active tab's panel (doc tabs share one
  // region, the Job-ad tab swaps in its own). Derive the active pair so the
  // panel can label itself back to whichever tab is selected.
  const activeTabKey = view === 'jobAd' ? 'jobad' : activeOut;
  const activeTabId = `tailor-tab-${activeTabKey}`;
  const activePanelId = `tailor-panel-${activeTabKey}`;

  type TabKey = 'resume' | 'cover' | 'jobad';
  const tabItems = useMemo<readonly TabItem<TabKey>[]>(() => {
    const docKeys: ('resume' | 'cover')[] = target === 'both' ? ['resume', 'cover'] : [activeOut];
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
  }, [target, activeOut, t]);

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
        {/* Filename + LIVE template picker strip (parity with the AI Generate done step).
            The single chosen template/ATS drive BOTH docs' preview + export, so the
            picker is shown on BOTH doc tabs (résumé AND cover) — never on the job-ad
            tab. The ATS-safe toggle sits beside it on whichever tab the flag can
            still change: the résumé (linearize / drop the photo) or a cover letter
            whose layout has a decoration to drop. */}
        {view === 'doc' && (
          <div className="shrink-0 flex flex-wrap items-center gap-2 border-b border-foreground/[0.06] px-3 py-1.5 text-[10px] text-foreground/30">
            {meta && (
              <>
                <FileText size={10} />
                <span className="font-mono">{buildFilename(meta, docType, 'pdf')}</span>
              </>
            )}
            <div className="ml-auto flex items-center gap-2">
              {/* Template dropdown — render-time switch (drives BOTH docs' preview +
                  export), no regeneration. Its list is portalled to <body> (fixed),
                  so the scrollport around it never clips the options. */}
              <div className="w-40">
                <Dropdown
                  id="template-picker"
                  options={templateOptions}
                  value={templateId}
                  onChange={handleTemplateChange}
                  icon={<LayoutTemplate size={11} />}
                  listClassName="max-h-48"
                />
              </div>
              {/* ATS-safe toggle — one flag, shown on whichever tab it can still
                  act on: the résumé tab for design-tier templates (two-column OR
                  photo, incl. Lebenslauf), and the cover tab when the letter's
                  layout carries a decoration ATS mode drops. The hint names the
                  document, so the same switch reads honestly on both tabs. */}
              {showAtsToggle && (
                <AtsModeToggle
                  checked={atsMode}
                  onChange={onAtsModeChange}
                  hintKey={
                    activeOut === 'cover'
                      ? 'aiGenerate.atsModeHintLetter'
                      : atsModeHintKey(templateId)
                  }
                />
              )}
              {/* The toggle APPEARS when a decorated layout (or a design-tier
                  template) is picked — a silent DOM insertion otherwise. This
                  region is always mounted (a live region only announces content
                  added AFTER it exists) and `sr-only` is out of flow, so the
                  toolbar's gap is unchanged. Pattern: SectionTimeline's announcer. */}
              <span role="status" aria-live="polite" className="sr-only">
                {showAtsToggle ? t('aiGenerate.atsToggleAvailable') : ''}
              </span>
            </div>
          </div>
        )}
        {/* Document-accent strip — render-time colour override; drives BOTH docs'
            preview + export, mirroring the template picker above. */}
        {view === 'doc' && (
          <div className="shrink-0 border-b border-foreground/[0.06] px-3 py-2">
            <AccentPicker value={accent} onChange={onAccentChange} />
          </div>
        )}
        {/* Letter-layout strip — cover-only (the layout only affects the letter; the
            résumé is unaffected). Drives the cover preview + export; picking a
            decorated layout here is what surfaces the ATS toggle above. */}
        {view === 'doc' && activeOut === 'cover' && (
          <div className="shrink-0 border-b border-foreground/[0.06] px-3 py-2">
            <LetterLayoutPicker value={letterLayoutId} onChange={handleLetterLayoutChange} />
          </div>
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
