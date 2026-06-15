import { ArrowLeft, ArrowRight, Monitor, Palette } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import {
  applyThemeAnimated,
  Button,
  cn,
  FloatingIcon,
  getThemePrefs,
  type ThemePrefs,
  withDelay,
} from '@ajh/ui';

import { ACCENTS, SCHEMES } from '@/constants/appearance';
import { useSystemAccent } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

interface Props {
  onBack?: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Final onboarding step: pick a colour scheme and accent. Reuses the same theme
 * engine as the Settings AppearanceCard — `applyThemeAnimated` persists, so no
 * preferences-store wiring is needed here. Text size and a11y switches are
 * intentionally omitted to keep the step lean; they live in Settings.
 */
export function AppearanceStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
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
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance
    >
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={Palette} size={24} />
      </div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.appearance.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.appearance.subtitle')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-6 space-y-4"
      >
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
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        {onBack && (
          <Button variant="ghost" onClick={onBack} className="flex items-center gap-1.5">
            <ArrowLeft size={13} />
            {t('onboarding.appearance.back')}
          </Button>
        )}

        <div className="flex-1" />

        <Button variant="primary" onClick={onNext} className="flex items-center gap-1.5">
          {t('onboarding.appearance.next')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
