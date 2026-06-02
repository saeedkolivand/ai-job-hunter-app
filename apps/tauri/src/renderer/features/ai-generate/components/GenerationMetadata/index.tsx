import { AlertTriangle, Globe } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { isCjkLanguage } from '@ajh/shared/language-detection';

import type { GenerationMeta } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

interface GenerationMetadataProps {
  meta: GenerationMeta | null;
}

export function GenerationMetadata({ meta }: GenerationMetadataProps) {
  const { t } = useTranslation();

  if (!meta) return null;

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0 }}
        className="mx-6 mb-4 rounded-xl border border-white/[0.07] bg-white/[0.02] px-4 py-3 space-y-2"
      >
        <div className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
          {t('aiGenerate.detectedContext')}
        </div>
        {[
          { label: t('aiGenerate.candidate'), value: meta.candidateName || '—' },
          { label: t('aiGenerate.role'), value: meta.jobTitle || '—' },
          { label: t('aiGenerate.company'), value: meta.companyName || '—' },
          {
            label: t('aiGenerate.languages'),
            value: meta.mismatch
              ? `${meta.resumeLanguage.toUpperCase()} resume → ${meta.jobAdLanguage.toUpperCase()} job ad`
              : meta.resumeLanguage.toUpperCase(),
          },
        ].map(({ label, value }) => (
          <div key={label} className="flex items-center justify-between">
            <span className="text-[11px] text-foreground/40">{label}</span>
            <span className="text-[11px] font-medium text-foreground/75 max-w-[200px] truncate text-right">
              {value}
            </span>
          </div>
        ))}
        {meta.mismatch && (
          <div className="flex items-center gap-1.5 rounded-lg border border-amber-400/15 bg-amber-400/[0.05] px-2.5 py-1.5 text-[10px] text-amber-300/80">
            <Globe size={10} />{' '}
            {t('aiGenerate.languageMismatch', { lang: meta.jobAdLanguage.toUpperCase() })}
          </div>
        )}
        {isCjkLanguage(meta.targetLanguage) && (
          <div className="flex items-start gap-1.5 rounded-lg border border-amber-400/15 bg-amber-400/[0.05] px-2.5 py-1.5 text-[10px] text-amber-300/80">
            <AlertTriangle size={10} className="mt-0.5 shrink-0" /> {t('aiGenerate.cjkUnsupported')}
          </div>
        )}
      </motion.div>
    </AnimatePresence>
  );
}
