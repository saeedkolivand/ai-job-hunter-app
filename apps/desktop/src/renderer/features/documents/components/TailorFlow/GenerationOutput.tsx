import { Check, Copy, Download, FileText, LayoutTemplate } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Dropdown, Switch, type TabItem, Tabs } from '@ajh/ui';

import { AccentPicker } from '@/components/generation/AccentPicker';
import { EditableOutput } from '@/components/generation/EditableOutput';
import { type ExportFormat, ExportPicker } from '@/components/generation/ExportPicker';
import { LetterLayoutPicker } from '@/components/generation/LetterLayoutPicker';
import { PdfPreview } from '@/components/generation/PdfPreview';
import { useDebouncedCommit } from '@/hooks/use-debounced-commit';
import {
  buildFilename,
  type GenerationMeta,
  isDesignTier,
  isTwoColumnTemplate,
  type LetterLayoutId,
  TEMPLATE_IDS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';

import { JobAdView } from './JobAdView';
import type { TailorTarget } from './useTailorGeneration';

interface Props {
  target: TailorTarget;
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  // Render-time template/ATS (sticky store) — drives BOTH the preview here and the
  // export in useTailorGeneration. The toolbar picker mutates them; no regeneration.
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
  const atsSwitchId = useId();

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

  // Template picker (mirrors GenerateWizard.handleTemplateChange): selecting an
  // ATS-tier template forces ATS off, since ATS-safe mode only applies to
  // design-tier layouts (two-column OR photo, incl. Lebenslauf). One template id
  // drives BOTH docs' preview + export.
  const templateOptions = TEMPLATE_IDS.map((id) => ({ value: id, label: TEMPLATES[id].name }));
  const handleTemplateChange = (value: string) => {
    const id = value as TemplateId;
    onTemplateChange(id);
    if (!isDesignTier(id)) onAtsModeChange(false);
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

  return (
    <div className="flex min-h-56 flex-1 flex-col rounded-lg border border-foreground/[0.06] bg-foreground/[0.02]">
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
      {/* Filename + LIVE template picker strip (parity with the AI Generate done step).
          The single chosen template/ATS drive BOTH docs' preview + export, so the
          picker is shown on BOTH doc tabs (résumé AND cover) — never on the job-ad
          tab. The ATS-safe toggle stays résumé-only (ATS single-column linearization
          is a résumé concept; cover letters aren't two-column). */}
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
                export), no regeneration. */}
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
            {/* ATS-safe toggle — résumé-only + only meaningful for design-tier
                templates (two-column OR photo, incl. Lebenslauf; copies
                StepTemplate's switch markup; reuses the aiGenerate.atsMode keys). */}
            {activeOut === 'resume' && isDesignTier(templateId) && (
              <div
                title={t(
                  isTwoColumnTemplate(templateId)
                    ? 'aiGenerate.atsModeHintTwoColumn'
                    : 'aiGenerate.atsModeHintPhoto'
                )}
                className={cn(
                  'flex h-auto items-center gap-1.5 rounded-lg border px-2 py-1 transition-all',
                  atsMode
                    ? 'border-brand/35 bg-brand/8 text-foreground/80'
                    : 'border-foreground/[0.06] bg-transparent text-foreground/45'
                )}
              >
                {/* Real <label htmlFor> so clicking the text toggles the switch
                    (whole-control click) and supplies its accessible name. */}
                <label
                  htmlFor={atsSwitchId}
                  className="cursor-pointer select-none text-[10px] font-medium"
                >
                  {t('aiGenerate.atsMode')}
                </label>
                <Switch
                  id={atsSwitchId}
                  size="sm"
                  checked={atsMode}
                  onCheckedChange={onAtsModeChange}
                />
              </div>
            )}
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
          résumé is unaffected, so it's the cover-doc counterpart to the résumé-only
          ATS toggle). Drives the cover preview + export. */}
      {view === 'doc' && activeOut === 'cover' && (
        <div className="shrink-0 border-b border-foreground/[0.06] px-3 py-2">
          <LetterLayoutPicker value={letterLayoutId} onChange={onLetterLayoutChange} />
        </div>
      )}
      <div
        role="tabpanel"
        id={activePanelId}
        aria-labelledby={activeTabId}
        tabIndex={0}
        className="flex min-h-[32rem] flex-1 flex-col px-3 py-2"
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
  );
}
