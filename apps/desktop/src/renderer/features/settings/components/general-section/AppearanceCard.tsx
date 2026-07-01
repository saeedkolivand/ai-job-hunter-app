import { Monitor, Palette, Type } from 'lucide-react';
import { useRef, useState } from 'react';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  applyThemeAnimated,
  Button,
  cn,
  getThemePrefs,
  SettingsSection,
  Switch,
  type TextScale,
  type ThemePrefs,
} from '@ajh/ui';

import { ACCENTS, SCHEMES } from '@/constants/appearance';
import { makeRovingTabindex } from '@/hooks/use-roving-tabindex';
import { useSystemAccent } from '@/services';

// Each button previews its own size via the text utility it sets.
const SCALES: { id: TextScale; labelKey: string; size: string }[] = [
  { id: 'small', labelKey: 'settings.appearance.textSmall', size: 'text-xs' },
  { id: 'default', labelKey: 'settings.appearance.textDefault', size: 'text-sm' },
  { id: 'large', labelKey: 'settings.appearance.textLarge', size: 'text-base' },
];

export function AppearanceCard() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<ThemePrefs>(() => getThemePrefs());
  // Only offered when the OS accent is readable (Windows/macOS); on Linux/read
  // failure `supported` is false and the System chip is hidden (no error UI).
  const { data: sysAccent } = useSystemAccent();

  const schemeRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const textSizeRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const accentRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const update = (patch: Partial<ThemePrefs>) => {
    const next = { ...prefs, ...patch };
    setPrefs(next);
    applyThemeAnimated(next);
  };

  // Build accent items list for roving tabindex (must match render order)
  const accentItems = [
    'default' as const,
    ...(sysAccent?.supported ? ['system' as const] : []),
    ...ACCENTS.map((a) => a.id),
  ];
  const currentAccentKey =
    prefs.accentSource === 'default'
      ? ('default' as const)
      : prefs.accentSource === 'system'
        ? ('system' as const)
        : (ACCENTS.find((a) => a.color.toLowerCase() === prefs.accentColor?.toLowerCase())?.id ??
          'default');

  const schemeIds = SCHEMES.map((s) => s.id);
  const scaleIds = SCALES.map((s) => s.id);

  return (
    <SettingsSection icon={Palette} label={t('settings.appearance.title')}>
      <div className="space-y-4">
        <div data-settings-anchor="appearance-theme">
          <div className="mb-2 text-xs font-medium text-foreground/55">
            {t('settings.appearance.scheme')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.scheme')}
            className="grid grid-cols-1 gap-2 @xs:grid-cols-3"
            onKeyDown={makeRovingTabindex(
              schemeIds,
              prefs.scheme,
              (v) => update({ scheme: v }),
              schemeRefs
            )}
          >
            {SCHEMES.map(({ id, icon: Icon, labelKey }, i) => {
              const active = prefs.scheme === id;
              return (
                <Button
                  key={id}
                  ref={(el) => {
                    schemeRefs.current[i] = el;
                  }}
                  role="radio"
                  aria-checked={active}
                  tabIndex={active ? 0 : -1}
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

        <div data-settings-anchor="appearance-accent">
          <div className="mb-2 text-xs font-medium text-foreground/55">
            {t('settings.appearance.accent')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.accent')}
            className="flex flex-wrap items-center gap-2"
            onKeyDown={makeRovingTabindex(
              accentItems,
              currentAccentKey,
              (v) => {
                if (v === 'default') update({ accentSource: 'default', accentColor: undefined });
                else if (v === 'system')
                  update({ accentSource: 'system', accentColor: sysAccent?.color ?? undefined });
                else {
                  const accent = ACCENTS.find((a) => a.id === v);
                  if (accent)
                    update({
                      accentSource: 'custom',
                      accentColor: accent.color,
                      accentColor2: accent.color2,
                    });
                }
              },
              accentRefs
            )}
          >
            <Button
              ref={(el) => {
                accentRefs.current[0] = el;
              }}
              variant="unstyled"
              role="radio"
              aria-checked={prefs.accentSource === 'default'}
              tabIndex={prefs.accentSource === 'default' ? 0 : -1}
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
                data-testid={TEST_IDS.settings.defaultAccentDot}
                className="h-3 w-3 rounded-full"
                style={{
                  // BASE brand tokens (never overridden by the runtime accent
                  // applier) so the Default dot always shows the true shipped
                  // default, even while a custom/system accent is active.
                  background:
                    'linear-gradient(135deg, var(--color-brand-base), var(--color-brand-mid-base), var(--color-brand-2-base))',
                }}
              />
              {t('settings.appearance.accentDefault')}
            </Button>
            {sysAccent?.supported && (
              <Button
                ref={(el) => {
                  accentRefs.current[1] = el;
                }}
                variant="unstyled"
                role="radio"
                aria-checked={prefs.accentSource === 'system'}
                tabIndex={prefs.accentSource === 'system' ? 0 : -1}
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
            {ACCENTS.map(({ id, color, color2, labelKey }, i) => {
              const active =
                prefs.accentSource === 'custom' &&
                prefs.accentColor?.toLowerCase() === color.toLowerCase();
              const refIdx = (sysAccent?.supported ? 2 : 1) + i;
              return (
                <Button
                  key={id}
                  ref={(el) => {
                    accentRefs.current[refIdx] = el;
                  }}
                  variant="unstyled"
                  role="radio"
                  aria-checked={active}
                  tabIndex={active ? 0 : -1}
                  aria-label={t(labelKey)}
                  title={t(labelKey)}
                  onClick={() =>
                    update({ accentSource: 'custom', accentColor: color, accentColor2: color2 })
                  }
                  className={cn(
                    'h-7 w-7 rounded-full border-2 transition-transform focus-visible:ring-2 focus-visible:ring-brand/50',
                    active ? 'scale-110 border-foreground/70' : 'border-transparent hover:scale-105'
                  )}
                  data-accent-color={color}
                  data-accent-color2={color2}
                  style={{ background: `linear-gradient(135deg, ${color}, ${color2})` }}
                />
              );
            })}
          </div>
        </div>

        <div data-settings-anchor="appearance-textsize">
          <div className="mb-2 flex items-center gap-1.5 text-xs font-medium text-foreground/55">
            <Type size={13} />
            {t('settings.appearance.textSize')}
          </div>
          <div
            role="radiogroup"
            aria-label={t('settings.appearance.textSize')}
            className="grid grid-cols-1 gap-2 @xs:grid-cols-3"
            onKeyDown={makeRovingTabindex(
              scaleIds,
              prefs.textScale,
              (v) => update({ textScale: v }),
              textSizeRefs
            )}
          >
            {SCALES.map(({ id, labelKey, size }, i) => {
              const active = prefs.textScale === id;
              return (
                <Button
                  key={id}
                  ref={(el) => {
                    textSizeRefs.current[i] = el;
                  }}
                  role="radio"
                  aria-checked={active}
                  tabIndex={active ? 0 : -1}
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

        <div data-settings-anchor="appearance-transparency">
          <Switch
            label={t('settings.appearance.reduceTransparency')}
            description={t('settings.appearance.reduceTransparencyHint')}
            checked={prefs.reduceTransparency}
            onCheckedChange={(v) => update({ reduceTransparency: v })}
          />
        </div>
        <div data-settings-anchor="appearance-contrast">
          <Switch
            label={t('settings.appearance.increaseContrast')}
            description={t('settings.appearance.increaseContrastHint')}
            checked={prefs.contrast === 'more'}
            onCheckedChange={(v) => update({ contrast: v ? 'more' : 'normal' })}
          />
        </div>
      </div>
    </SettingsSection>
  );
}
