import { AlertTriangle, Info, Layers, Sparkles, Zap } from 'lucide-react';

import { type TFunction, useTranslation } from '@ajh/translations';
import { Button, HoverPopover, SegmentedControl, type SegmentedOption } from '@ajh/ui';

import {
  ALL_GENERATION_DEPTHS,
  type GenerationDepth,
  isDepthRunnable,
} from '@/lib/generate/generation-depth';

const DEPTH_ICON = { fast: Zap, quality: Sparkles, max: Layers } as const;

export interface DepthSelectorProps {
  value: GenerationDepth;
  onChange: (depth: GenerationDepth) => void;
  /**
   * True when the active model looks small AND local. Shows the soft warning
   * below — never disables anything (plan decision: honest copy, never block;
   * a parse failure degrades through the re-ask into needs-review, it does not
   * corrupt anything).
   */
  smallModel?: boolean;
  /**
   * Why quality depth can't run from THIS surface, if it can't (e.g. the
   * generation has no saved résumé or no cached posting to resolve server-side).
   * Rendered as an honest note; the option stays selectable so the preference
   * still records what the user wants.
   */
  unavailableReason?: string;
  /** Compact layout for a wizard step (no section label). */
  hideLabel?: boolean;
  className?: string;
}

/**
 * Per-option `title`, the repo's badge-describe convention: every option says
 * what it does on hover/long-press without the user opening anything. The info
 * popover next to the control carries the longer story for all three.
 */
function optionTitle(depth: GenerationDepth, t: TFunction): string {
  return isDepthRunnable(depth)
    ? t(`generationDepth.option.${depth}.tooltip`)
    : t('generationDepth.option.max.unavailable');
}

/**
 * Depth picker + the "what do these mean" popover.
 *
 * Three depths are always SHOWN even though this build runs two: `max` is
 * rendered disabled with a reason rather than hidden, because a two-option
 * control silently claims there are two depths. `resume_pipeline_run` rejects
 * `depth: "max"` outright, so offering it as selectable would be the lie.
 */
export function DepthSelector({
  value,
  onChange,
  smallModel,
  unavailableReason,
  hideLabel,
  className,
}: DepthSelectorProps) {
  const { t } = useTranslation();

  const options: SegmentedOption<GenerationDepth>[] = ALL_GENERATION_DEPTHS.map((depth) => ({
    value: depth,
    label: t(`generationDepth.option.${depth}.label`),
    icon: DEPTH_ICON[depth],
    title: optionTitle(depth, t),
    ...(isDepthRunnable(depth) ? {} : { disabled: true }),
  }));

  return (
    <div className={className}>
      <div className="mb-2 flex items-center gap-1.5">
        {!hideLabel && (
          <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('generationDepth.label')}
          </span>
        )}
        <HoverPopover
          ariaLabel={t('generationDepth.info.title')}
          contentClassName="max-w-xs"
          trigger={
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-label={t('generationDepth.info.open')}
              className="h-5 w-5 p-0 text-foreground/40 hover:text-foreground/70"
            >
              <Info size={12} />
            </Button>
          }
        >
          <div className="space-y-2.5 text-[11px] leading-relaxed text-foreground/70">
            <p className="text-[11px] font-semibold text-foreground/90">
              {t('generationDepth.info.title')}
            </p>
            {ALL_GENERATION_DEPTHS.map((depth) => (
              <div key={depth}>
                <p className="font-medium text-foreground/85">
                  {t(`generationDepth.option.${depth}.label`)}
                  {!isDepthRunnable(depth) && (
                    <span className="ml-1.5 text-[10px] font-normal text-foreground/40">
                      {t('generationDepth.info.notYet')}
                    </span>
                  )}
                </p>
                <p className="text-foreground/55">{t(`generationDepth.info.${depth}.what`)}</p>
                <p className="text-foreground/45">{t(`generationDepth.info.${depth}.cost`)}</p>
              </div>
            ))}
            <p className="border-t border-white/[0.08] pt-2 text-foreground/45">
              {t('generationDepth.info.report')}
            </p>
          </div>
        </HoverPopover>
      </div>

      <SegmentedControl
        options={options}
        value={value}
        onChange={onChange}
        variant="grid"
        ariaLabel={t('generationDepth.label')}
      />

      <p className="mt-1.5 text-[10px] leading-relaxed text-foreground/40">
        {t(`generationDepth.option.${value}.summary`)}
      </p>

      {value === 'quality' && unavailableReason && (
        <p role="status" className="mt-1.5 text-[10px] leading-relaxed text-amber-300/70">
          {unavailableReason}
        </p>
      )}

      {value !== 'fast' && smallModel && (
        <div
          role="status"
          className="mt-2 flex items-start gap-2 rounded-xl border border-amber-500/20 bg-amber-500/5 px-3 py-2"
        >
          <AlertTriangle size={12} className="mt-0.5 shrink-0 text-amber-400" />
          <p className="text-[10px] leading-relaxed text-amber-400/80">
            {t('generationDepth.smallModelWarning')}
          </p>
        </div>
      )}
    </div>
  );
}
