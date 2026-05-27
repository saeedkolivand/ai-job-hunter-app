import { Check } from 'lucide-react';
import { motion } from 'motion/react';

import { LOCALES } from '@/constants/locales';
import i18n from '@/i18n';
import { cn } from '@ajh/ui';
import { usePreferencesStore } from '@/store/preferences-store';

export function LanguageSelector() {
  const setLanguage = usePreferencesStore((s) => s.setLanguage);
  const current = i18n.language;

  const select = (code: string) => {
    void i18n.changeLanguage(code);
    setLanguage(code);
  };

  return (
    <div className="grid grid-cols-3 gap-2">
      {LOCALES.map(({ code, label, flag }) => {
        const active = current === code;
        return (
          <motion.button
            key={code}
            onClick={() => select(code)}
            className={cn(
              'relative flex items-center gap-2.5 rounded-lg border px-3 py-2.5 text-left text-xs transition-all duration-150',
              active
                ? 'border-brand/40 bg-brand/10 text-foreground/90'
                : 'border-white/[0.06] bg-white/[0.02] text-foreground/55 hover:border-white/10 hover:bg-white/[0.05] hover:text-foreground/80'
            )}
          >
            <span className="text-base leading-none">{flag}</span>
            <span className="flex-1 font-medium">{label}</span>
            {active && <Check size={11} className="shrink-0 text-brand-soft" />}
          </motion.button>
        );
      })}
    </div>
  );
}
