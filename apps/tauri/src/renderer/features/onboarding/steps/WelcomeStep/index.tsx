import { ArrowRight, Check, Wand2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, FloatingIcon, Input } from '@ajh/ui';

import { LOCALES } from '@/constants/locales';
import i18n from '@/i18n';
import { usePreferencesStore } from '@/store/preferences-store';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

interface Props {
  onNext: () => void;
  onBack?: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

export function WelcomeStep({ onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const setUserName = usePreferencesStore((s) => s.setUserName);
  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const storedName = usePreferencesStore((s) => s.userName);
  const storedLanguage = usePreferencesStore((s) => s.language);
  const [name, setName] = useState(storedName ?? '');
  const inputRef = useRef<HTMLInputElement>(null);
  const currentLang = storedLanguage ?? i18n.language;

  const selectLanguage = (code: string) => {
    void i18n.changeLanguage(code);
    setLanguage(code);
  };

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleNext = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setUserName(trimmed);
    onNext();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.stopPropagation();
      handleNext();
    }
  };

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onNext={onNext}
      canAdvance={name.trim().length > 0}
      className="max-w-md"
    >
      {/* Icon */}
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={Wand2} size={24} />
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
        <label className="text-xs font-semibold uppercase tracking-widest text-foreground/55">
          {t('onboarding.welcome.nameLabel')}
        </label>
        <Input
          ref={inputRef}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('onboarding.welcome.namePlaceholder')}
          className="w-full text-base"
        />
      </div>

      {/* Language */}
      <div className="mb-8 space-y-2">
        <label className="text-xs font-semibold uppercase tracking-widest text-foreground/55">
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

      <Button
        variant="default"
        size="lg"
        className="w-full justify-center"
        onClick={handleNext}
        disabled={name.trim().length === 0}
      >
        {t('onboarding.welcome.next')}
        <ArrowRight size={15} />
      </Button>
    </OnboardingStepWrapper>
  );
}
