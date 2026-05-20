import { motion } from 'motion/react';
import { Sparkles } from 'lucide-react';
import { useTranslation } from '@/lib/i18n';

interface OutputPanelExtractingProps {
  stageLabel: string;
}

export function OutputPanelExtracting({ stageLabel }: OutputPanelExtractingProps) {
  const { t } = useTranslation();

  return (
    <motion.div
      key="extracting"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex flex-1 flex-col items-center justify-center gap-8"
    >
      <div className="relative flex h-24 w-24 items-center justify-center">
        <div className="absolute inset-0 rounded-full border-2 border-brand/20 animate-ping" />
        <div className="absolute inset-2 rounded-full border border-brand/30 animate-pulse" />
        <Sparkles size={28} className="text-brand-soft" />
      </div>
      <div className="text-center">
        <div className="text-sm font-medium text-foreground/70">{stageLabel}</div>
        <div className="mt-1 text-xs text-foreground/35">{t('aiGenerate.analyzingDocuments')}</div>
      </div>
    </motion.div>
  );
}
