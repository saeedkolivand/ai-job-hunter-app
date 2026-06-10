import { Check, Copy, Download, FileText, LayoutTemplate } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { Button, cn, SelectDropdown } from '@ajh/ui';

import { EditableOutput } from '@/components/generation/EditableOutput';
import { PdfPreview } from '@/components/generation/PdfPreview';
import {
  buildFilename,
  type GenerationMeta,
  isTwoColumnTemplate,
  TEMPLATE_IDS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import type { TailorTarget } from './useTailorGeneration';

interface Props {
  target: TailorTarget;
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  // Render-time template/ATS (sticky store) — drives BOTH the preview here and the
  // export in useTailorGeneration. The toolbar picker mutates them; no regeneration.
  templateId: TemplateId;
  atsMode: boolean;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (v: boolean) => void;
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
}

export function GenerationOutput({
  target,
  activeOut,
  setActiveOut,
  templateId,
  atsMode,
  onTemplateChange,
  onAtsModeChange,
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
}: Props) {
  const { t } = useTranslation();
  const [view, setView] = useState<'doc' | 'jobAd'>('doc');

  // Committed text per doc — what PdfPreview renders (recompiles only on discrete
  // events: generation/regenerate/tab-switch/Save, never per keystroke). This
  // component only ever sees the ACTIVE doc's `output`, so we key by `activeOut`.
  const [committed, setCommitted] = useState<Record<'resume' | 'cover', string>>({
    resume: activeOut === 'resume' ? output : '',
    cover: activeOut === 'cover' ? output : '',
  });
  const lastEditRef = useRef<Record<'resume' | 'cover', string | null>>({
    resume: null,
    cover: null,
  });

  // Refresh committed for the active doc when `output` changes for a reason OTHER
  // than a local edit (generation, regenerate, or a tab switch that swaps `output`
  // to the other doc's canonical text). A local edit sets lastEditRef so it does
  // NOT refresh — the preview waits for Save.
  useEffect(() => {
    if (output !== lastEditRef.current[activeOut]) {
      setCommitted((c) => ({ ...c, [activeOut]: output }));
      lastEditRef.current[activeOut] = null;
    }
  }, [output, activeOut]);

  const handleEdit = (value: string) => {
    lastEditRef.current[activeOut] = value;
    onEdit(value);
  };
  const docType = activeOut === 'resume' ? 'resume' : 'cover-letter';

  // Template picker (mirrors GenerateWizard.handleTemplateChange): selecting a
  // single-column template forces ATS off, since ATS-safe mode only linearizes the
  // two-column templates. One template id drives BOTH docs' preview + export.
  const templateOptions = TEMPLATE_IDS.map((id) => ({ value: id, label: TEMPLATES[id].name }));
  const handleTemplateChange = (value: string) => {
    const id = value as TemplateId;
    onTemplateChange(id);
    if (!isTwoColumnTemplate(id)) onAtsModeChange(false);
  };

  // ARIA tabs contract: each tab owns a stable id and controls a panel id; the
  // single content region below is the active tab's panel (doc tabs share one
  // region, the Job-ad tab swaps in its own). Derive the active pair so the
  // panel can label itself back to whichever tab is selected.
  const activeTabKey = view === 'jobAd' ? 'jobad' : activeOut;
  const activeTabId = `tailor-tab-${activeTabKey}`;
  const activePanelId = `tailor-panel-${activeTabKey}`;

  return (
    <div className="flex min-h-56 flex-1 flex-col rounded-lg border border-white/[0.06] bg-white/[0.02]">
      <div className="shrink-0 flex items-center justify-between border-b border-white/[0.06] px-3 py-2">
        <div role="tablist" className="flex items-center gap-1">
          {(target === 'both' ? (['resume', 'cover'] as const) : ([activeOut] as const)).map(
            (o) => (
              <Button
                key={o}
                id={`tailor-tab-${o}`}
                variant="unstyled"
                type="button"
                role="tab"
                aria-selected={view === 'doc' && activeOut === o}
                aria-controls={`tailor-panel-${o}`}
                onClick={() => {
                  setView('doc');
                  setActiveOut(o);
                }}
                className={cn(
                  'rounded px-2 py-0.5 text-[10px] font-medium transition-colors',
                  view === 'doc' && activeOut === o
                    ? 'bg-brand/15 text-brand-soft'
                    : 'text-foreground/40 hover:text-foreground/70'
                )}
              >
                {o === 'resume'
                  ? t('autopilot.apply.target.resume')
                  : t('autopilot.apply.target.cover')}
              </Button>
            )
          )}
          <Button
            id="tailor-tab-jobad"
            variant="unstyled"
            type="button"
            role="tab"
            aria-selected={view === 'jobAd'}
            aria-controls="tailor-panel-jobad"
            onClick={() => setView('jobAd')}
            className={cn(
              'rounded px-2 py-0.5 text-[10px] font-medium transition-colors',
              view === 'jobAd'
                ? 'bg-brand/15 text-brand-soft'
                : 'text-foreground/40 hover:text-foreground/70'
            )}
          >
            {t('autopilot.apply.tabs.jobAd')}
          </Button>
        </div>
        <div className="flex items-center gap-3">
          <Button
            onClick={() => void onCopy()}
            disabled={!output || view === 'jobAd'}
            className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-foreground/40 hover:text-foreground/70"
          >
            {copied ? <Check size={11} /> : <Copy size={11} />}
            {copied ? t('autopilot.apply.copied') : t('autopilot.apply.copy')}
          </Button>
          <div className="relative">
            <Button
              onClick={() => setExportOpen((o) => !o)}
              disabled={!output || view === 'jobAd'}
              className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-brand-soft hover:text-brand-soft/80"
            >
              <Download size={11} />
              {t('aiGenerate.export')}
            </Button>
            {exportOpen && (
              <>
                <div className="fixed inset-0 z-[650]" onClick={() => setExportOpen(false)} />
                <div className="absolute right-0 top-full z-[700] mt-1.5 w-32 overflow-hidden rounded-lg border border-white/10 bg-secondary shadow-2xl">
                  {(['pdf', 'docx', 'txt'] as const).map((fmt) => (
                    <Button
                      key={fmt}
                      variant="unstyled"
                      type="button"
                      onClick={() => void onExport(fmt)}
                      className="flex w-full items-center gap-2 px-3 py-2 text-[11px] text-foreground/65 transition-colors hover:bg-white/[0.05] hover:text-foreground"
                    >
                      <Download size={10} />
                      {t('aiGenerate.download', { fmt: fmt.toUpperCase() })}
                    </Button>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
      {/* Filename + LIVE template picker strip (parity with the AI Generate done step).
          The single chosen template/ATS drive BOTH docs' preview + export, so the
          picker is shown on BOTH doc tabs (résumé AND cover) — never on the job-ad
          tab. The ATS-safe toggle stays résumé-only (ATS single-column linearization
          is a résumé concept; cover letters aren't two-column). */}
      {view === 'doc' && (
        <div className="shrink-0 flex flex-wrap items-center gap-2 border-b border-white/[0.06] px-3 py-1.5 text-[10px] text-foreground/30">
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
              <SelectDropdown
                options={templateOptions}
                value={templateId}
                onChange={handleTemplateChange}
                icon={<LayoutTemplate size={11} />}
                listClassName="max-h-48"
              />
            </div>
            {/* ATS-safe toggle — résumé-only + only meaningful for two-column
                templates (copies StepTemplate's switch markup; reuses the
                aiGenerate.atsMode keys). */}
            {activeOut === 'resume' && isTwoColumnTemplate(templateId) && (
              <Button
                variant="unstyled"
                type="button"
                role="switch"
                aria-checked={atsMode}
                onClick={() => onAtsModeChange(!atsMode)}
                title={t('aiGenerate.atsModeHint')}
                className={cn(
                  'flex h-auto items-center gap-1.5 rounded-lg border px-2 py-1 transition-all',
                  atsMode
                    ? 'border-brand/35 bg-brand/8 text-foreground/80'
                    : 'border-white/[0.06] bg-transparent text-foreground/45 hover:border-white/10'
                )}
              >
                <span className="text-[10px] font-medium">{t('aiGenerate.atsMode')}</span>
                <span
                  className={cn(
                    'relative h-4 w-7 shrink-0 rounded-full transition-colors',
                    atsMode ? 'bg-brand' : 'bg-white/10'
                  )}
                >
                  <span
                    className={cn(
                      'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                      atsMode ? 'translate-x-3.5' : 'translate-x-0.5'
                    )}
                  />
                </span>
              </Button>
            )}
          </div>
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
          <div className="flex-1 select-text overflow-y-auto whitespace-pre-wrap text-[11px] leading-relaxed text-foreground/60">
            {jobDesc || t('autopilot.apply.noDescription')}
          </div>
        ) : (
          <EditableOutput
            value={output}
            onChange={handleEdit}
            onSave={() => setCommitted((c) => ({ ...c, [activeOut]: output }))}
            canSave={output !== committed[activeOut]}
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
