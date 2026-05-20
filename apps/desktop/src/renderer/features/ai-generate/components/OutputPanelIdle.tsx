import { Wand2 } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@/lib/i18n';

export function OutputPanelIdle() {
  const { t } = useTranslation();

  return (
    <motion.div
      key="idle"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex flex-1 flex-col items-center justify-center gap-6 px-10"
    >
      <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/20">
        <Wand2 size={36} className="text-brand-soft/60" />
      </div>
      <div className="text-center">
        <div className="text-lg font-semibold text-foreground/50">
          {t('aiGenerate.yourAICareerCopilot')}
        </div>
        <div className="mt-1 text-sm text-foreground/30">{t('aiGenerate.pasteResumeJobAd')}</div>
      </div>
      <div className="flex flex-col gap-2 text-center">
        {[
          t('aiGenerate.features.atsOptimized'),
          t('aiGenerate.features.personalizedCover'),
          t('aiGenerate.features.multilingual'),
          t('aiGenerate.features.smartExport'),
        ].map((f, i) => (
          <div key={i} className="flex items-center gap-2 text-xs text-foreground/35">
            <span className="h-1 w-1 rounded-full bg-brand/40" /> {f}
          </div>
        ))}
      </div>
    </motion.div>
  );
}
