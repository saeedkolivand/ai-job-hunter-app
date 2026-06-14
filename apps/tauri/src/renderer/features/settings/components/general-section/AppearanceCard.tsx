import { Monitor, Moon, Palette, Sun, Type } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import {
  applyThemeAnimated,
  Button,
  cn,
  type ColorScheme,
  getThemePrefs,
  SettingsSection,
  Switch,
  type TextScale,
  type ThemePrefs,
} from '@ajh/ui';

import { useSystemAccent } from '@/services';

const SCHEMES: { id: ColorScheme; icon: typeof Sun; labelKey: string }[] = [
  { id: 'light', icon: Sun, labelKey: 'settings.appearance.light' },
  { id: 'dark', icon: Moon, labelKey: 'settings.appearance.dark' },
  { id: 'system', icon: Monitor, labelKey: 'settings.appearance.system' },
];

// Each button previews its own size via the text utility it sets.
const SCALES: { id: TextScale; labelKey: string; size: string }[] = [
  { id: 'small', labelKey: 'settings.appearance.textSmall', size: 'text-xs' },
  { id: 'default', labelKey: 'settings.appearance.textDefault', size: 'text-sm' },
  { id: 'large', labelKey: 'settings.appearance.textLarge', size: 'text-base' },
];

// macOS-style preset accents the user can pick manually. 'default' (handled
// separately) keeps the shipped, per-scheme-tuned violet; each preset here is a
// fixed hex applied to both schemes via the theme engine's accent applier.
const ACCENTS: { id: string; color: string; color2: string; labelKey: string }[] = [
  {
    id: 'violet',
    color: '#a855f7',
    color2: '#6366f1',
    labelKey: 'settings.appearance.accentViolet',
  },
  { id: 'blue', color: '#007aff', color2: '#22d3ee', labelKey: 'settings.appearance.accentBlue' },
  { id: 'green', color: '#34c759', color2: '#06b6a4', labelKey: 'settings.appearance.accentGreen' },
  {
    id: 'orange',
    color: '#ff9500',
    color2: '#ffb340',
    labelKey: 'settings.appearance.accentOrange',
  },
  { id: 'pink', color: '#ff2d55', color2: '#ff5e9c', labelKey: 'settings.appearance.accentPink' },
  { id: 'red', color: '#ff3b30', color2: '#ff2d7a', labelKey: 'settings.appearance.accentRed' },
  {
    id: 'yellow',
    color: '#ffcc00',
    color2: '#ff9500',
    labelKey: 'settings.appearance.accentYellow',
  },
  {
    id: 'graphite',
    color: '#8e8e93',
    color2: '#6e7280',
    labelKey: 'settings.appearance.accentGraphite',
  },
];

export function AppearanceCard() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<ThemePrefs>(() => getThemePrefs());
  // Only offered when the OS accent is readable (Windows/macOS); on Linux/read
  // failure `supported` is false and the System chip is hidden (no error UI).
  const { data: sysAccent } = useSystemAccent();

  const update = (patch: Partial<ThemePrefs>) => {
    const next = { ...prefs, ...patch };
    setPrefs(next);
    applyThemeAnimated(next);
  };

  return (
    <SettingsSection icon={Palette} label={t('settings.appearance.title')}>
      <div className="space-y-4">
        <div>
          <div className="mb-2 text-xs font-medium text-foreground/55">
            {t('settings.appearance.scheme')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.scheme')}
            className="grid grid-cols-3 gap-2"
          >
            {SCHEMES.map(({ id, icon: Icon, labelKey }) => {
              const active = prefs.scheme === id;
              return (
                <Button
                  key={id}
                  role="radio"
                  aria-checked={active}
                  onClick={() => update({ scheme: id })}
                  className={cn(
                    'flex h-auto flex-col items-center gap-1.5 rounded-xl border px-3 py-3 text-xs font-medium transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
                    active
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-foreground/10 bg-foreground/[0.02] text-foreground/55 hover:text-foreground/80'
                  )}
                >
                  <Icon size={16} />
                  {t(labelKey)}
                </Button>
              );
            })}
          </div>
        </div>

        <div>
          <div className="mb-2 text-xs font-medium text-foreground/55">
            {t('settings.appearance.accent')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.accent')}
            className="flex flex-wrap items-center gap-2"
          >
            <Button
              variant="unstyled"
              role="radio"
              aria-checked={prefs.accentSource === 'default'}
              aria-label={t('settings.appearance.accentDefault')}
              title={t('settings.appearance.accentDefault')}
              onClick={() => update({ accentSource: 'default', accentColor: undefined })}
              className={cn(
                'flex h-7 items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
                prefs.accentSource === 'default'
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-foreground/10 text-foreground/55 hover:text-foreground/80'
              )}
            >
              <span
                className="h-3 w-3 rounded-full"
                style={{
                  background: 'linear-gradient(135deg, var(--color-brand), var(--color-brand-2))',
                }}
              />
              {t('settings.appearance.accentDefault')}
            </Button>
            {sysAccent?.supported && (
              <Button
                variant="unstyled"
                role="radio"
                aria-checked={prefs.accentSource === 'system'}
                aria-label={t('settings.appearance.system')}
                title={t('settings.appearance.system')}
                onClick={() =>
                  update({ accentSource: 'system', accentColor: sysAccent.color ?? undefined })
                }
                className={cn(
                  'flex h-7 items-center gap-1.5 rounded-full border px-3 text-xs font-medium transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
                  prefs.accentSource === 'system'
                    ? 'border-brand/40 bg-brand/10 text-brand-soft'
                    : 'border-foreground/10 text-foreground/55 hover:text-foreground/80'
                )}
              >
                <Monitor size={12} />
                {t('settings.appearance.system')}
              </Button>
            )}
            {ACCENTS.map(({ id, color, color2, labelKey }) => {
              const active =
                prefs.accentSource === 'custom' &&
                prefs.accentColor?.toLowerCase() === color.toLowerCase();
              return (
                <Button
                  key={id}
                  variant="unstyled"
                  role="radio"
                  aria-checked={active}
                  aria-label={t(labelKey)}
                  title={t(labelKey)}
                  onClick={() =>
                    update({ accentSource: 'custom', accentColor: color, accentColor2: color2 })
                  }
                  className={cn(
                    'h-7 w-7 rounded-full border-2 transition-transform focus-visible:ring-2 focus-visible:ring-brand/50',
                    active ? 'scale-110 border-foreground/70' : 'border-transparent hover:scale-105'
                  )}
                  style={{ background: `linear-gradient(135deg, ${color}, ${color2})` }}
                />
              );
            })}
          </div>
        </div>

        <div>
          <div className="mb-2 flex items-center gap-1.5 text-xs font-medium text-foreground/55">
            <Type size={13} />
            {t('settings.appearance.textSize')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.textSize')}
            className="grid grid-cols-3 gap-2"
          >
            {SCALES.map(({ id, labelKey, size }) => {
              const active = prefs.textScale === id;
              return (
                <Button
                  key={id}
                  role="radio"
                  aria-checked={active}
                  onClick={() => update({ textScale: id })}
                  className={cn(
                    'flex h-auto items-center justify-center rounded-xl border px-3 py-2.5 font-medium transition-all focus-visible:ring-2 focus-visible:ring-brand/50',
                    size,
                    active
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-foreground/10 bg-foreground/[0.02] text-foreground/55 hover:text-foreground/80'
                  )}
                >
                  {t(labelKey)}
                </Button>
              );
            })}
          </div>
        </div>

        <Switch
          label={t('settings.appearance.reduceTransparency')}
          description={t('settings.appearance.reduceTransparencyHint')}
          checked={prefs.reduceTransparency}
          onCheckedChange={(v) => update({ reduceTransparency: v })}
        />
        <Switch
          label={t('settings.appearance.increaseContrast')}
          description={t('settings.appearance.increaseContrastHint')}
          checked={prefs.contrast === 'more'}
          onCheckedChange={(v) => update({ contrast: v ? 'more' : 'normal' })}
        />
      </div>
    </SettingsSection>
  );
}
