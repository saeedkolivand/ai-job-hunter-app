import { useId, useRef, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Input } from '@ajh/ui';

import { makeRovingTabindex } from '@/hooks/use-roving-tabindex';

/** A 6-hex colour, with or without a leading `#`. */
const HEX_RE = /^#?[0-9a-fA-F]{6}$/;

/**
 * Curated professional **document accents** (distinct from the app-UI accent of
 * ADR 0004 — this recolours the exported résumé/letter, never the app chrome).
 * Each is a fixed hex; `undefined` (the "Template default" chip) leaves the
 * chosen template's own palette untouched.
 */
const DOCUMENT_ACCENTS = [
  { id: 'navy', color: '#1B3A5C', labelKey: 'aiGenerate.documentAccentNavy' },
  { id: 'slate', color: '#46505C', labelKey: 'aiGenerate.documentAccentSlate' },
  { id: 'burgundy', color: '#6E1E2B', labelKey: 'aiGenerate.documentAccentBurgundy' },
  { id: 'forest', color: '#1B4332', labelKey: 'aiGenerate.documentAccentForest' },
  { id: 'teal', color: '#1A5C52', labelKey: 'aiGenerate.documentAccentTeal' },
  { id: 'copper', color: '#A0522D', labelKey: 'aiGenerate.documentAccentCopper' },
  { id: 'steel', color: '#4A6785', labelKey: 'aiGenerate.documentAccentSteel' },
] as const;

interface AccentPickerProps {
  /** Current accent hex (`#RRGGBB`) or `undefined` for the template default. */
  value?: string;
  /** Fired with a curated/custom hex, or `undefined` when cleared to default. */
  onChange: (accent: string | undefined) => void;
  className?: string;
}

/**
 * Per-export document-accent control: a "Template default" chip + a row of
 * curated swatches + a custom 6-hex input. The swatch fills are DATA (inline
 * style, like `AppearanceCard`'s preset swatches); all control chrome uses
 * design tokens. Not persisted and never reads ThemePrefs — a malformed custom
 * value is kept in the field but not propagated (the backend also falls back to
 * the template palette), so the export stays predictable.
 */
export function AccentPicker({ value, onChange, className }: AccentPickerProps) {
  const { t } = useTranslation();
  const customId = useId();
  const swatchRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const matched = value
    ? DOCUMENT_ACCENTS.find((a) => a.color.toLowerCase() === value.toLowerCase())
    : undefined;
  const isCustom = Boolean(value) && !matched;
  const [customText, setCustomText] = useState(isCustom ? (value ?? '') : '');

  // Roving-tabindex item order MUST match the rendered button order.
  const items = ['default', ...DOCUMENT_ACCENTS.map((a) => a.id)];
  const currentKey = value === undefined ? 'default' : (matched?.id ?? 'custom');
  // A custom hex ('custom') sits OUTSIDE the radio set, so it can't own the
  // single tab stop. Fall back to the "Template default" chip: the radiogroup
  // always keeps exactly one tabbable element (APG roving-tabindex), and arrow
  // keys advance from a real in-set position instead of stalling at index -1.
  const rovingKey = currentKey === 'custom' ? 'default' : currentKey;

  const selectDefault = () => {
    setCustomText('');
    onChange(undefined);
  };
  const selectSwatch = (color: string) => {
    setCustomText('');
    onChange(color);
  };
  const handleCustom = (raw: string) => {
    setCustomText(raw);
    const trimmed = raw.trim();
    if (trimmed.length === 0) {
      // Cleared field is an explicit reset to the template default, matching the
      // aria state (no radio selected) — no keyboard dead-end back to default.
      onChange(undefined);
    } else if (HEX_RE.test(trimmed)) {
      onChange(`#${trimmed.replace(/^#/, '').toUpperCase()}`);
    }
    // Invalid / partial input: keep what the user typed but don't propagate.
  };

  const customInvalid = customText.trim().length > 0 && !HEX_RE.test(customText.trim());

  return (
    <div className={cn('space-y-2', className)}>
      <div className="text-xs font-medium text-foreground/55">{t('aiGenerate.documentAccent')}</div>

      <div
        role="radiogroup"
        aria-label={t('aiGenerate.documentAccent')}
        className="flex flex-wrap items-center gap-2"
        onKeyDown={makeRovingTabindex(
          items,
          rovingKey,
          (v) => {
            if (v === 'default') selectDefault();
            else {
              const a = DOCUMENT_ACCENTS.find((x) => x.id === v);
              if (a) selectSwatch(a.color);
            }
          },
          swatchRefs
        )}
      >
        <Button
          ref={(el) => {
            swatchRefs.current[0] = el;
          }}
          variant="unstyled"
          role="radio"
          aria-checked={currentKey === 'default'}
          tabIndex={rovingKey === 'default' ? 0 : -1}
          onClick={selectDefault}
          data-testid={TEST_IDS.generation.accentDefault}
          className={cn(
            'flex h-7 items-center rounded-full border px-3 text-xs font-medium transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
            currentKey === 'default'
              ? 'border-brand/40 bg-brand/10 text-brand-soft'
              : 'border-foreground/10 text-foreground/55 hover:text-foreground/80'
          )}
        >
          {t('aiGenerate.documentAccentDefault')}
        </Button>

        {DOCUMENT_ACCENTS.map((a, i) => {
          const active = matched?.id === a.id;
          return (
            <Button
              key={a.id}
              ref={(el) => {
                swatchRefs.current[i + 1] = el;
              }}
              variant="unstyled"
              role="radio"
              aria-checked={active}
              tabIndex={active ? 0 : -1}
              aria-label={t(a.labelKey)}
              title={t(a.labelKey)}
              onClick={() => selectSwatch(a.color)}
              data-testid={`${TEST_IDS.generation.accentSwatch}-${a.id}`}
              className={cn(
                'h-7 w-7 rounded-full border-2 transition-transform focus-visible:ring-2 focus-visible:ring-brand/50',
                active ? 'scale-110 border-foreground/70' : 'border-transparent hover:scale-105'
              )}
              // The swatch fill is data (a curated hex), not brand chrome — inline
              // style mirrors AppearanceCard's preset swatches.
              style={{ background: a.color }}
            />
          );
        })}
      </div>

      <div className="flex items-center gap-2">
        <label htmlFor={customId} className="text-[11px] text-foreground/45">
          {t('aiGenerate.documentAccentCustom')}
        </label>
        <Input
          id={customId}
          value={customText}
          onChange={(e) => handleCustom(e.target.value)}
          placeholder="#1B3A5C"
          spellCheck={false}
          aria-invalid={customInvalid || undefined}
          data-testid={TEST_IDS.generation.accentCustom}
          className={cn('w-28 font-mono text-xs', isCustom && 'border-brand/40')}
        />
        {isCustom && (
          <span
            aria-hidden
            className="h-5 w-5 shrink-0 rounded-full border border-foreground/20"
            style={{ background: value }}
          />
        )}
      </div>
    </div>
  );
}
