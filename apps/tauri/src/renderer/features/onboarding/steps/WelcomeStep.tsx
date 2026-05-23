import { ArrowRight, Check, Wand2 } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { Button, Input } from '@ajh/ui';

import { LOCALES } from '@/constants/locales';
import i18n from '@/i18n';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { usePreferencesStore } from '@/store/preferences-store';

interface Props {
  onNext: () => void;
  direction: number;
}

export function WelcomeStep({ onNext, direction }: Props) {
  const { t } = useTranslation();
  const setUserName = usePreferencesStore((s) => s.setUserName);
  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const storedName = usePreferencesStore((s) => s.userName);
  const [name, setName] = useState(storedName ?? '');
  const inputRef = useRef<HTMLInputElement>(null);
  const currentLang = i18n.language;

  const selectLanguage = (code: string) => {
    void i18n.changeLanguage(code);
    setLanguage(code);
  };

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleNext = () => {
    const trimmed = name.trim();
    if (trimmed) setUserName(trimmed);
    onNext();
  };

  return (
    <motion.div
      className="relative z-10 w-full max-w-md mx-4"
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
        {/* Icon */}
        <div className="mb-6 flex justify-center">
          <div
            className="flex h-14 w-14 items-center justify-center rounded-2xl"
            style={{
              background:
                'linear-gradient(135deg, rgba(168,85,247,0.25) 0%, rgba(99,102,241,0.15) 100%)',
              border: '1px solid rgba(168,85,247,0.3)',
              boxShadow: '0 0 32px rgba(168,85,247,0.2)',
            }}
          >
            <Wand2 size={24} className="text-brand-soft" />
          </div>
        </div>

        {/* Heading */}
        <div className="mb-8 text-center">
          <h1 className="mb-2 text-xl font-semibold text-foreground/95">
            {t('onboarding.welcome.title')}
          </h1>
          <p className="text-sm text-foreground/50">{t('onboarding.welcome.subtitle')}</p>
        </div>

        {/* Name input */}
        <div className="mb-6 space-y-2">
          <label className="text-xs font-medium uppercase tracking-widest text-foreground/35">
            {t('onboarding.welcome.nameLabel')}
          </label>
          <Input
            ref={inputRef}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleNext()}
            placeholder={t('onboarding.welcome.namePlaceholder')}
            className="w-full text-base"
          />
        </div>

        {/* Language */}
        <div className="mb-8 space-y-2">
          <label className="text-xs font-medium uppercase tracking-widest text-foreground/35">
            {t('onboarding.prefs.languageLabel')}
          </label>
          <div className="grid grid-cols-4 gap-1.5 mt-1">
            {LOCALES.map(({ code, label, flag }) => {
              const active = currentLang === code;
              return (
                <div key={code} className="relative">
                  <Button
                    onClick={() => selectLanguage(code)}
                    className={cn(
                      'flex items-center gap-2 rounded-lg border px-2.5 py-2 text-left text-xs transition-all duration-150 h-auto w-full',
                      active
                        ? 'border-brand/40 bg-brand/10 text-foreground/90'
                        : 'border-white/[0.06] bg-white/[0.02] text-foreground/55 hover:border-white/10 hover:bg-white/[0.05] hover:text-foreground/80'
                    )}
                  >
                    <span className="text-sm leading-none">{flag}</span>
                    <span className="flex-1 truncate font-medium">{label}</span>
                  </Button>
                  {active && (
                    <div className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-brand text-white">
                      <Check size={8} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Step dots */}
        <div className="mb-6 flex justify-center gap-1.5">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className={`h-1 rounded-full transition-all duration-300 ${
                i === 0 ? 'w-5 bg-brand' : 'w-1.5 bg-white/15'
              }`}
            />
          ))}
        </div>

        <Button variant="default" size="lg" className="w-full justify-center" onClick={handleNext}>
          {t('onboarding.welcome.next')}
          <ArrowRight size={15} />
        </Button>
      </div>
    </motion.div>
  );
}
