import { Check } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { LOCALES } from '@/constants/locales';
import { usePreferencesStore } from '@/store/preferences-store';

const MotionButton = motion.create(Button);

export function LanguageSelector() {
  const { i18n } = useTranslation();
  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const current = i18n.language;

  const select = (code: string) => {
    void i18n.changeLanguage(code);
    setLanguage(code);
  };

  return (
    <div className="grid grid-cols-1 gap-2 @xs:grid-cols-3">
      {LOCALES.map(({ code, label, flag }) => {
        const active = current === code;
        return (
          <MotionButton
            key={code}
            variant="unstyled"
            type="button"
            onClick={() => select(code)}
            className={cn(
              'relative flex items-center gap-2.5 rounded-lg border px-3 py-2.5 text-left text-xs transition-all duration-150',
              active
                ? 'border-brand/40 bg-brand/10 text-foreground/90'
                : 'border-foreground/10 bg-foreground/[0.03] text-foreground/55 hover:border-foreground/20 hover:bg-foreground/[0.06] hover:text-foreground/80'
            )}
          >
            <span className="text-base leading-none">{flag}</span>
            <span className="flex-1 font-medium">{label}</span>
            {active && <Check size={11} className="shrink-0 text-brand-soft" />}
          </MotionButton>
        );
      })}
    </div>
  );
}
