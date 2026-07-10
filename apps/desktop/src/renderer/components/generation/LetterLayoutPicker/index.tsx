import { AlignLeft, type LucideIcon, PanelTop, PenLine } from 'lucide-react';
import { useRef } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { makeRovingTabindex } from '@/hooks/use-roving-tabindex';
import { LETTER_LAYOUT_IDS, type LetterLayoutId } from '@/lib/generate';

interface LayoutOption {
  id: LetterLayoutId;
  icon: LucideIcon;
  labelKey: string;
  descKey: string;
}

/**
 * The three cover-letter layouts. Order mirrors {@link LETTER_LAYOUT_IDS} so the
 * roving-tabindex item order matches the rendered order. A layout owns only the
 * ARRANGEMENT — its palette/fonts inherit from the chosen résumé template.
 */
const LAYOUTS = [
  {
    id: 'classic',
    icon: AlignLeft,
    labelKey: 'aiGenerate.letterLayoutClassic',
    descKey: 'aiGenerate.letterLayoutClassicDesc',
  },
  {
    id: 'refined',
    icon: PenLine,
    labelKey: 'aiGenerate.letterLayoutRefined',
    descKey: 'aiGenerate.letterLayoutRefinedDesc',
  },
  {
    id: 'banded',
    icon: PanelTop,
    labelKey: 'aiGenerate.letterLayoutBanded',
    descKey: 'aiGenerate.letterLayoutBandedDesc',
  },
] as const satisfies readonly LayoutOption[];

interface LetterLayoutPickerProps {
  /** Current layout, or `undefined` (treated as `classic`, the backend default). */
  value?: LetterLayoutId;
  /** Fired with the chosen layout id. */
  onChange: (id: LetterLayoutId) => void;
  className?: string;
}

/**
 * Per-export **letter-layout** control: a labeled radiogroup of the three
 * arrangements (Classic / Refined / Banded). Selection is always in-set — an
 * unset `value` shows `classic` selected — so the APG roving-tabindex has exactly
 * one real tab stop and arrows never stall (simpler than {@link AccentPicker},
 * whose custom-hex value can sit outside the radio set). Text + icon options
 * only; there are no layout preview images yet. Not persisted — the value is a
 * per-export choice threaded to the preview + export so both agree.
 */
export function LetterLayoutPicker({ value, onChange, className }: LetterLayoutPickerProps) {
  const { t } = useTranslation();
  const optionRefs = useRef<(HTMLButtonElement | null)[]>([]);

  // Unset → `classic` (the backend default), so exactly one radio is always checked.
  const selected: LetterLayoutId = value ?? 'classic';

  return (
    <div className={cn('space-y-2', className)}>
      <div className="text-xs font-medium text-foreground/55">{t('aiGenerate.letterLayout')}</div>

      <div
        role="radiogroup"
        aria-label={t('aiGenerate.letterLayout')}
        className="flex flex-col gap-1.5"
        onKeyDown={makeRovingTabindex(LETTER_LAYOUT_IDS, selected, onChange, optionRefs)}
      >
        {LAYOUTS.map((layout, i) => {
          const active = selected === layout.id;
          const Icon = layout.icon;
          return (
            <Button
              key={layout.id}
              ref={(el) => {
                optionRefs.current[i] = el;
              }}
              variant="unstyled"
              role="radio"
              aria-checked={active}
              tabIndex={active ? 0 : -1}
              onClick={() => onChange(layout.id)}
              data-testid={`${TEST_IDS.generation.letterLayoutOption}-${layout.id}`}
              className={cn(
                'flex h-auto items-start gap-2.5 rounded-xl border px-3 py-2 text-left transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
                active
                  ? 'border-brand/50 bg-brand/10'
                  : 'border-[var(--border-clear)] bg-card hover:bg-muted'
              )}
            >
              <Icon
                size={15}
                aria-hidden
                className={cn('mt-0.5 shrink-0', active ? 'text-brand-soft' : 'text-foreground/40')}
              />
              <span className="flex min-w-0 flex-col">
                <span
                  className={cn(
                    'text-[12px] font-medium leading-tight',
                    active ? 'text-foreground/90' : 'text-foreground/60'
                  )}
                >
                  {t(layout.labelKey)}
                </span>
                <span className="mt-0.5 text-[10px] leading-tight text-foreground/40">
                  {t(layout.descKey)}
                </span>
              </span>
            </Button>
          );
        })}
      </div>
    </div>
  );
}
