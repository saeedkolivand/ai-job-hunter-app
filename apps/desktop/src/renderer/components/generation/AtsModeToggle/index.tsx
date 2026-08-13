import { useId } from 'react';

import { useTranslation } from '@ajh/translations';
import { cn, Switch } from '@ajh/ui';

import type { AtsModeHintKey } from '@/lib/generate';

interface AtsModeToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /**
   * i18n key for the hint describing what the toggle changes for the document
   * being shown — `atsModeHintKey(templateId)` for a résumé,
   * `aiGenerate.atsModeHintLetter` for a cover letter. The union (not `string`)
   * so a typo'd key is a `tsc` failure rather than a raw key on screen.
   *
   * Reaches the user twice: as the switch's accessible DESCRIPTION (the Switch
   * primitive's `description`, exposed via aria-describedby — the copy names its
   * document, which is the whole point on a surface showing two of them) and as
   * the hover `title` for sighted users.
   */
  hintKey: AtsModeHintKey | 'aiGenerate.atsModeHintLetter';
  className?: string;
}

/**
 * Compact **ATS-safe mode** switch for a document toolbar strip.
 *
 * One flag, two documents: the résumé linearizes / drops its photo (a no-op for
 * ATS-tier templates) and a decorated cover-letter layout drops its rail, tile
 * or band. Each caller decides whether the flag can act on the document it is
 * showing and picks the matching `hintKey`; this component only renders the
 * control, so the two toolbar surfaces (TailorFlow's GenerationOutput and AI
 * Generate's OutputPanelDone) cannot drift apart in markup or a11y wiring.
 *
 * StepTemplate keeps its own full-width card switch — a wizard step has the room
 * for a heading + hint lines, a toolbar strip does not.
 */
export function AtsModeToggle({ checked, onChange, hintKey, className }: AtsModeToggleProps) {
  const { t } = useTranslation();
  const switchId = useId();

  return (
    <div
      title={t(hintKey)}
      className={cn(
        'flex h-auto items-center gap-1.5 rounded-lg border px-2 py-1 transition-all',
        checked
          ? 'border-brand/35 bg-brand/8 text-foreground/80'
          : 'border-foreground/[0.06] bg-transparent text-foreground/45',
        className
      )}
    >
      {/* Real <label htmlFor> so clicking the text toggles the switch
          (whole-control click) and supplies its accessible name. */}
      <label htmlFor={switchId} className="cursor-pointer select-none text-[10px] font-medium">
        {t('aiGenerate.atsMode')}
      </label>
      {/* `description` (not just the wrapper's `title`, which AT never reads off
          a role-less div): the label alone says "ATS-safe mode" without saying
          WHICH document it changes — on a two-document surface that is the only
          part that matters. Visually hidden here; the title covers hover. */}
      <Switch
        id={switchId}
        size="sm"
        checked={checked}
        onCheckedChange={onChange}
        description={t(hintKey)}
      />
    </div>
  );
}
