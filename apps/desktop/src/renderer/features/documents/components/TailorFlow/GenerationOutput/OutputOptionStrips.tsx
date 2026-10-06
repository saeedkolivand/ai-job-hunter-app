import { FileText, LayoutTemplate } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Dropdown } from '@ajh/ui';

import { AccentPicker } from '@/components/generation/AccentPicker';
import { AtsModeToggle } from '@/components/generation/AtsModeToggle';
import { LetterLayoutPicker } from '@/components/generation/LetterLayoutPicker';
import {
  atsModeHintKey,
  buildFilename,
  type GenerationMeta,
  isDecoratedLetterLayout,
  isDesignTier,
  type LetterLayoutId,
  shouldClearAtsMode,
  TEMPLATE_IDS,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';

import type { TailorTarget } from '../lib/tailor-target';

interface OutputOptionStripsProps {
  target: TailorTarget;
  /** Does the panel show a résumé tab at all (see `GenerationOutput`)? */
  resumeInRun: boolean;
  activeOut: 'resume' | 'cover';
  docType: 'resume' | 'cover-letter';
  meta: GenerationMeta | null;
  templateId: TemplateId;
  atsMode: boolean;
  accent?: string;
  letterLayoutId?: LetterLayoutId;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (v: boolean) => void;
  onAccentChange: (accent: string | undefined) => void;
  onLetterLayoutChange: (id: LetterLayoutId) => void;
}

/** The render-time option strips above the document: template + ATS, accent, and (cover) letter layout. */
export function OutputOptionStrips({
  target,
  resumeInRun,
  activeOut,
  docType,
  meta,
  templateId,
  atsMode,
  accent,
  letterLayoutId,
  onTemplateChange,
  onAtsModeChange,
  onAccentChange,
  onLetterLayoutChange,
}: OutputOptionStripsProps) {
  const { t } = useTranslation();

  // Does the one ATS-safe flag still act on this export's cover letter? True
  // only for a run that produces a letter whose layout carries a decoration
  // (band / rail / monogram tile) the renderer drops under `data.opts.ats`.
  const letterAtsApplies = target !== 'resume' && isDecoratedLetterLayout(letterLayoutId);

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

  return (
    <>
      {/* Filename + LIVE template picker strip (parity with the AI Generate done step).
          The single chosen template/ATS drive BOTH docs' preview + export, so the
          picker is shown on BOTH doc tabs (résumé AND cover) — never on the job-ad
          tab. The ATS-safe toggle sits beside it on whichever tab the flag can
          still change: the résumé (linearize / drop the photo) or a cover letter
          whose layout has a decoration to drop. */}
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
                activeOut === 'cover' ? 'aiGenerate.atsModeHintLetter' : atsModeHintKey(templateId)
              }
            />
          )}
          {/* The toggle APPEARS when a decorated layout (or a design-tier
              template) is picked — a silent DOM insertion otherwise. This
              region is always mounted (a live region only announces content
              added AFTER it exists) and `sr-only` is out of flow, so the
              toolbar's gap is unchanged. */}
          <span role="status" aria-live="polite" className="sr-only">
            {showAtsToggle ? t('aiGenerate.atsToggleAvailable') : ''}
          </span>
        </div>
      </div>
      {/* Document-accent strip — render-time colour override; drives BOTH docs'
          preview + export, mirroring the template picker above. */}
      <div className="shrink-0 border-b border-foreground/[0.06] px-3 py-2">
        <AccentPicker value={accent} onChange={onAccentChange} />
      </div>
      {/* Letter-layout strip — cover-only (the layout only affects the letter; the
          résumé is unaffected). Drives the cover preview + export; picking a
          decorated layout here is what surfaces the ATS toggle above. */}
      {activeOut === 'cover' && (
        <div className="shrink-0 border-b border-foreground/[0.06] px-3 py-2">
          <LetterLayoutPicker value={letterLayoutId} onChange={handleLetterLayoutChange} />
        </div>
      )}
    </>
  );
}
