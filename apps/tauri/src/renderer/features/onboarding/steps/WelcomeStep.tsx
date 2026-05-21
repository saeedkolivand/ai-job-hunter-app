import { ArrowRight, Wand2 } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { Button, Input } from '@ajh/ui';

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
  const storedName = usePreferencesStore((s) => s.userName);
  const [name, setName] = useState(storedName ?? '');
  const inputRef = useRef<HTMLInputElement>(null);

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
        <div className="mb-8 space-y-2">
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

        {/* Step dots */}
        <div className="mb-6 flex justify-center gap-1.5">
          {[0, 1, 2, 3].map((i) => (
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
