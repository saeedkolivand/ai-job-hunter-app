import { Monitor, Moon, Palette, Sun } from 'lucide-react';
import { useState } from 'react';

import {
  applyTheme,
  Button,
  cn,
  type ColorScheme,
  getThemePrefs,
  GlassCard,
  IconBadge,
  SectionLabel,
  type ThemePrefs,
} from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

const SCHEMES: { id: ColorScheme; icon: typeof Sun; labelKey: string }[] = [
  { id: 'light', icon: Sun, labelKey: 'settings.appearance.light' },
  { id: 'dark', icon: Moon, labelKey: 'settings.appearance.dark' },
  { id: 'system', icon: Monitor, labelKey: 'settings.appearance.system' },
];

function ToggleRow({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0">
        <div className="text-xs font-medium text-foreground/80">{label}</div>
        <div className="text-[11px] text-foreground/45">{hint}</div>
      </div>
      <Button
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={() => onChange(!checked)}
        className={cn(
          'relative h-5 w-9 shrink-0 rounded-full border-transparent p-0 transition-colors focus-visible:ring-2 focus-visible:ring-brand/50',
          checked ? 'bg-brand' : 'bg-foreground/15'
        )}
      >
        <span
          className={cn(
            'absolute top-0.5 h-4 w-4 rounded-full bg-white shadow-sm transition-transform',
            checked ? 'translate-x-4' : 'translate-x-0.5'
          )}
        />
      </Button>
    </div>
  );
}

export function AppearanceCard() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<ThemePrefs>(() => getThemePrefs());

  const update = (patch: Partial<ThemePrefs>) => {
    const next = { ...prefs, ...patch };
    setPrefs(next);
    applyTheme(next);
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <IconBadge icon={Palette} size="sm" />
        <SectionLabel>{t('settings.appearance.title')}</SectionLabel>
      </div>

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

        <ToggleRow
          label={t('settings.appearance.reduceTransparency')}
          hint={t('settings.appearance.reduceTransparencyHint')}
          checked={prefs.reduceTransparency}
          onChange={(v) => update({ reduceTransparency: v })}
        />
        <ToggleRow
          label={t('settings.appearance.increaseContrast')}
          hint={t('settings.appearance.increaseContrastHint')}
          checked={prefs.contrast === 'more'}
          onChange={(v) => update({ contrast: v ? 'more' : 'normal' })}
        />
      </div>
    </GlassCard>
  );
}
