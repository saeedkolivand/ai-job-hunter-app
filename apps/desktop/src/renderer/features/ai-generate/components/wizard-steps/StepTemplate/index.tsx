import { Check, FileText } from 'lucide-react';
import { useId } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Image } from '@ajh/ui';

import { AccentPicker } from '@/components/generation/AccentPicker';
import { LetterLayoutPicker } from '@/components/generation/LetterLayoutPicker';
import {
  atsModeHintKey,
  isDecoratedLetterLayout,
  isDesignTier,
  type LetterLayoutId,
  shouldClearAtsMode,
  type TemplateId,
  TEMPLATES,
} from '@/lib/generate';

import { COVER_TEMPLATE_PREVIEWS, TEMPLATE_CAPTIONS, TEMPLATE_PREVIEWS } from '../../../samples';

interface StepTemplateProps {
  templateId: TemplateId;
  atsMode: boolean;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  target?: 'resume' | 'cover' | 'both';
  /** Per-export document accent (6-hex) — undefined = template palette. */
  accent?: string;
  /**
   * When provided, the document-accent picker is shown under the gallery. Left
   * out by surfaces that don't thread an accent (e.g. the résumé builder).
   */
  onAccentChange?: (accent: string | undefined) => void;
  /** Per-export cover-letter layout — undefined = classic (the backend default). */
  letterLayoutId?: LetterLayoutId;
  /**
   * When provided, the letter-layout picker is shown for any run that produces
   * a cover letter (`target === 'cover' | 'both'`) under the accent picker. Left
   * out by surfaces that never generate a cover letter (e.g. the résumé builder).
   */
  onLetterLayoutChange?: (id: LetterLayoutId) => void;
}

export function StepTemplate({
  templateId,
  atsMode,
  onTemplateChange,
  onAtsModeChange,
  target = 'resume',
  accent,
  onAccentChange,
  letterLayoutId,
  onLetterLayoutChange,
}: StepTemplateProps) {
  const isCover = target === 'cover';
  // 'both' also produces a cover letter, so it gets the picker up front too —
  // only a résumé-only run has no letter to lay out.
  const showLetterLayout = target !== 'resume';
  const { t } = useTranslation();

  // What the one ATS-safe flag actually drives in THIS run. The résumé half is
  // the original gate (design-tier layouts linearize / drop the photo); the
  // letter half is new: a decorated layout drops its rail / tile / band. Either
  // one is reason enough to show the switch — under an ATS-tier résumé template
  // the letter is the only thing the flag still affects, and before this the
  // toggle was unreachable in exactly that case.
  const resumeAtsApplies = !isCover && isDesignTier(templateId);
  const letterAtsApplies = showLetterLayout && isDecoratedLetterLayout(letterLayoutId);
  // One switch driving two documents is only honest if the user is told so —
  // shown solely in the ambiguous case, where both hint lines are up.
  const atsDrivesBothDocs = resumeAtsApplies && letterAtsApplies;

  // The hints are `aria-describedby` (never part of the accessible NAME, which
  // is the aria-label below) — mirrors the @ajh/ui Switch primitive, whose
  // description is exposed once via describedby rather than concatenated into
  // the name. Each id is emitted under the SAME guard as the line it points at.
  const hintBaseId = useId();
  const resumeHintId = `${hintBaseId}-resume`;
  const letterHintId = `${hintBaseId}-letter`;
  const bothHintId = `${hintBaseId}-both`;
  const atsDescribedBy =
    [
      resumeAtsApplies ? resumeHintId : null,
      letterAtsApplies ? letterHintId : null,
      atsDrivesBothDocs ? bothHintId : null,
    ]
      .filter((id): id is string => id !== null)
      .join(' ') || undefined;

  const handleTemplateSelect = (id: TemplateId) => {
    onTemplateChange(id);
    // ATS-tier templates have no ATS toggle (they are already parser-safe), so
    // clear any stale atsMode — UNLESS a decorated cover letter in this run is
    // still reading the flag, in which case clearing it would silently restore
    // the letter's decoration with no way back. Cover-only runs never clear.
    if (!isCover && shouldClearAtsMode(id, letterAtsApplies)) {
      onAtsModeChange(false);
    }
  };

  const handleLetterLayoutSelect = (id: LetterLayoutId) => {
    onLetterLayoutChange?.(id);
    // The mirror image of handleTemplateSelect, and the reason it exists: the
    // flag is SHARED, so leaving it set after the letter stops reading it means
    // the next decorated layout comes back silently pre-ATS'd — the user picks
    // Monogram and exports a letter with no monogram. Same two-input guard,
    // with the NEW layout's decoratedness: clear only when nothing is left to
    // read the flag (a design-tier résumé template still legitimately does).
    if (shouldClearAtsMode(templateId, isDecoratedLetterLayout(id))) {
      onAtsModeChange(false);
    }
  };

  const renderCard = (tpl: (typeof TEMPLATES)[TemplateId]) => {
    // 'both' (and 'resume') intentionally use the résumé thumbnails — the template is shared
    // and the résumé is the primary document; only cover-only swaps to cover-letter style previews.
    const image = isCover ? COVER_TEMPLATE_PREVIEWS[tpl.id] : TEMPLATE_PREVIEWS[tpl.id];
    const caption = t(TEMPLATE_CAPTIONS[tpl.id]);
    const selected = templateId === tpl.id;

    return (
      <Button
        key={tpl.id}
        onClick={() => handleTemplateSelect(tpl.id)}
        className={cn(
          'flex flex-col items-start gap-1.5 rounded-xl border p-2 text-left transition-all h-auto',
          selected
            ? 'border-brand/50 bg-brand/10 ring-2 ring-brand/40'
            : 'border-[var(--border-clear)] bg-card hover:bg-muted'
        )}
      >
        {/* Thumbnail */}
        <div className="relative w-full rounded-lg overflow-hidden bg-muted aspect-[3/4] flex items-center justify-center">
          {image ? (
            <Image
              src={image}
              alt={tpl.name}
              preview={false}
              rootClassName="w-full h-full"
              className="w-full h-full object-cover rounded-lg"
            />
          ) : (
            <FileText size={20} className="text-foreground/20" />
          )}
          {/* Tier badge — top-left, opposite the selected check. Static label. */}
          <span className="absolute left-1.5 top-1.5 rounded-full border border-[var(--border-clear)] bg-card/90 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wide text-foreground/60">
            {t(tpl.tier === 'design' ? 'aiGenerate.tier.designBadge' : 'aiGenerate.tier.atsBadge')}
          </span>
          {/* #13 — selected state reads clearly with a check badge. */}
          {selected && (
            <span className="absolute right-1.5 top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-brand text-white shadow-sm">
              <Check size={12} strokeWidth={3} />
            </span>
          )}
        </div>

        {/* Template name */}
        <span
          className={cn(
            'text-[10px] font-medium leading-tight w-full text-center',
            selected ? 'text-foreground/90' : 'text-foreground/50'
          )}
        >
          {tpl.name}
        </span>

        {caption && (
          <span className="text-[9px] text-foreground/30 leading-tight w-full text-center line-clamp-1">
            {caption}
          </span>
        )}
      </Button>
    );
  };

  const atsTemplates = Object.values(TEMPLATES).filter((tpl) => !isDesignTier(tpl.id));
  const designTemplates = Object.values(TEMPLATES).filter((tpl) => isDesignTier(tpl.id));

  return (
    <div className="space-y-4">
      {/* Template thumbnail gallery, grouped by tier (ATS-Safe first, then Design). */}
      <div className="space-y-1.5">
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
          {t('aiGenerate.tier.atsSafe')}
        </h3>
        <div className="grid grid-cols-1 gap-3 @xs:grid-cols-3">{atsTemplates.map(renderCard)}</div>
      </div>

      <div className="space-y-1.5">
        <h3 className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
          {t('aiGenerate.tier.design')}
        </h3>
        <div className="grid grid-cols-1 gap-3 @xs:grid-cols-3">
          {designTemplates.map(renderCard)}
        </div>
      </div>

      {/* Document accent — per-export colour override, threaded to preview + export. */}
      {onAccentChange && <AccentPicker value={accent} onChange={onAccentChange} />}

      {/* Letter layout — arrangement picker for any run that produces a cover
          letter ('cover' or 'both'; palette/fonts still inherit the résumé
          template), threaded to the cover preview + export. */}
      {showLetterLayout && onLetterLayoutChange && (
        <LetterLayoutPicker value={letterLayoutId} onChange={handleLetterLayoutSelect} />
      )}

      {/* ATS safe mode toggle — one flag for the whole export. Shown for
          design-tier résumé templates (two-column OR photo, incl. Lebenslauf)
          AND for a run whose cover letter uses a decorated layout; each hint
          line below states which document it changes. */}
      {(resumeAtsApplies || letterAtsApplies) && (
        <Button
          variant="unstyled"
          type="button"
          role="switch"
          aria-checked={atsMode}
          // Name = the label alone; the hint lines are descriptions (see above).
          aria-label={t('aiGenerate.atsMode')}
          aria-describedby={atsDescribedBy}
          onClick={() => onAtsModeChange(!atsMode)}
          className={cn(
            // items-START, not center: with two (German, free-wrapping) hint
            // lines the row grows to ~80–90px and a centred track floats far
            // below the label it belongs to. The track stays on the label's line.
            'w-full flex items-start justify-between rounded-xl border px-3 py-2.5 transition-all text-left',
            atsMode
              ? 'border-brand/35 bg-brand/8'
              : 'border-[var(--border-clear)] bg-card hover:bg-muted'
          )}
        >
          <div>
            <div
              className={cn(
                'text-[11px] font-medium',
                atsMode ? 'text-foreground/90' : 'text-foreground/55'
              )}
            >
              {t('aiGenerate.atsMode')}
            </div>
            {resumeAtsApplies && (
              <div id={resumeHintId} className="text-[10px] text-foreground/35 mt-0.5">
                {t(atsModeHintKey(templateId))}
              </div>
            )}
            {letterAtsApplies && (
              <div id={letterHintId} className="text-[10px] text-foreground/35 mt-0.5">
                {t('aiGenerate.atsModeHintLetter')}
              </div>
            )}
            {atsDrivesBothDocs && (
              <div id={bothHintId} className="text-[10px] text-foreground/45 mt-1">
                {t('aiGenerate.atsModeHintBothDocs')}
              </div>
            )}
          </div>
          <div
            className={cn(
              'h-4 w-7 rounded-full transition-colors shrink-0 ml-3 relative',
              atsMode ? 'bg-brand' : 'bg-muted'
            )}
          >
            <div
              className={cn(
                'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                atsMode ? 'translate-x-3.5' : 'translate-x-0.5'
              )}
            />
          </div>
        </Button>
      )}
    </div>
  );
}
