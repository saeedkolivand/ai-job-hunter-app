import { Monitor, Moon, Palette, Sun, Type } from 'lucide-react';
import { useState } from 'react';

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

import { useTranslation } from '@/lib/i18n';

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

export function AppearanceCard() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<ThemePrefs>(() => getThemePrefs());

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
