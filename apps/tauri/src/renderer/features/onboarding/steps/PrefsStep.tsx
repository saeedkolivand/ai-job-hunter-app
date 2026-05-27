import { ArrowLeft, ArrowRight, Check } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect } from 'react';

import { Button } from '@ajh/ui';

import { LOCALES } from '@/constants/locales';
import i18n from '@/i18n';
import { cn } from '@ajh/ui';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@ajh/ui';
import { useJobPreferences, useSetJobPreferences } from '@/services';
import { usePreferencesStore } from '@/store/preferences-store';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
}

const REMOTE_OPTIONS: { id: string; emoji: string }[] = [
  { id: 'remote', emoji: '🌍' },
  { id: 'hybrid', emoji: '🏠' },
  { id: 'on-site', emoji: '🏢' },
  { id: 'any', emoji: '✨' },
];

export function PrefsStep({ onBack, onNext, direction }: Props) {
  const { t } = useTranslation();
  const { data: jobPrefs } = useJobPreferences();
  const setJobPreferences = useSetJobPreferences();
  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const currentLang = i18n.language;

  const selectLanguage = (code: string) => {
    void i18n.changeLanguage(code);
    setLanguage(code);
  };

  const selectRemote = (remote: string) => {
    setJobPreferences.mutate({
      ...jobPrefs,
      remote,
    });
  };

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === 'ArrowRight' || e.key === 'ArrowDown') onNext();
      if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') onBack();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onNext, onBack]);

  return (
    <motion.div
      className="relative z-10 w-full max-w-lg mx-4"
      custom={direction}
      variants={{
        initial: (dir: number) => ({ opacity: 0, x: dir * 60 }),
        animate: { opacity: 1, x: 0 },
        exit: (dir: number) => ({ opacity: 0, x: dir * -60 }),
      }}
      initial="initial"
      animate="animate"
      exit="exit"
      transition={transition.modal}
    >
      <div
        className="rounded-2xl border border-white/[0.08] p-8"
        style={{
          background: 'linear-gradient(145deg, rgba(20,14,36,0.97) 0%, rgba(12,10,24,0.97) 100%)',
          boxShadow:
            '0 32px 80px rgba(0,0,0,0.6), 0 0 0 1px rgba(168,85,247,0.08), inset 0 1px 0 rgba(255,255,255,0.04)',
        }}
      >
        {/* Heading */}
        <div className="mb-6">
          <h2 className="mb-1 text-lg font-semibold text-foreground/95">
            {t('onboarding.prefs.title')}
          </h2>
          <p className="text-sm text-foreground/45">{t('onboarding.prefs.subtitle')}</p>
        </div>

        {/* Language */}
        <div className="mb-6">
          <div className="mb-2.5 text-[10px] font-semibold uppercase tracking-widest text-foreground/35">
            {t('onboarding.prefs.languageLabel')}
          </div>
          <div className="grid grid-cols-4 gap-1.5">
            {LOCALES.map(({ code, label, flag }) => {
              const active = currentLang === code;
              return (
                <Button
                  key={code}
                  onClick={() => selectLanguage(code)}
                  className={cn(
                    'flex items-center gap-2 rounded-lg border px-2.5 py-2 text-left text-xs transition-all duration-150 h-auto',
                    active
                      ? 'border-brand/40 bg-brand/10 text-foreground/90'
                      : 'border-white/[0.06] bg-white/[0.02] text-foreground/55 hover:border-white/10 hover:bg-white/[0.05] hover:text-foreground/80'
                  )}
                >
                  <span className="text-sm leading-none">{flag}</span>
                  <span className="flex-1 truncate font-medium">{label}</span>
                  {active && <Check size={9} className="shrink-0 text-brand-soft" />}
                </Button>
              );
            })}
          </div>
        </div>

        {/* Remote preference */}
        <div className="mb-8">
          <div className="mb-2.5 text-[10px] font-semibold uppercase tracking-widest text-foreground/35">
            {t('onboarding.prefs.remoteLabel')}
          </div>
          <div className="grid grid-cols-4 gap-1.5">
            {REMOTE_OPTIONS.map(({ id, emoji }) => {
              const active = jobPrefs?.remote === id;
              return (
                <Button
                  key={id}
                  onClick={() => selectRemote(id)}
                  className={cn(
                    'flex flex-col items-center gap-1.5 rounded-xl border px-2 py-3 text-center transition-all duration-150 h-auto',
                    active
                      ? 'border-brand/40 bg-brand/10'
                      : 'border-white/[0.06] bg-white/[0.02] hover:border-white/10 hover:bg-white/[0.05]'
                  )}
                >
                  <span className="text-xl leading-none">{emoji}</span>
                  <span
                    className={cn(
                      'text-[11px] font-medium leading-tight',
                      active ? 'text-foreground/90' : 'text-foreground/50'
                    )}
                  >
                    {t(`onboarding.prefs.remote.${id}`)}
                  </span>
                </Button>
              );
            })}
          </div>
        </div>

        {/* Step dots */}
        <div className="mb-6 flex justify-center gap-1.5">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`h-1 rounded-full transition-all duration-300 ${
                i === 1 ? 'w-5 bg-brand' : 'w-1.5 bg-white/15'
              }`}
            />
          ))}
        </div>

        {/* Actions */}
        <div className="flex gap-2">
          <Button variant="ghost" size="lg" className="flex-1 justify-center" onClick={onBack}>
            <ArrowLeft size={14} />
            {t('onboarding.prefs.back')}
          </Button>
          <Button variant="default" size="lg" className="flex-1 justify-center" onClick={onNext}>
            {t('onboarding.prefs.next')}
            <ArrowRight size={14} />
          </Button>
        </div>
      </div>
    </motion.div>
  );
}
