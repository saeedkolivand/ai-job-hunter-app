import { Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';

interface OutputPanelGeneratingProps {
  stageLabel: string;
  streamBuffer: string;
  activeOut: 'resume' | 'cover';
}

export function OutputPanelGenerating({
  stageLabel,
  streamBuffer,
  activeOut,
}: OutputPanelGeneratingProps) {
  const { t } = useTranslation();

  return (
    <motion.div
      key="generating"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Stage label */}
      <div className="shrink-0 border-b border-white/[0.05] px-6 py-4">
        <div className="flex items-center gap-3">
          <Loader2 size={14} className="animate-spin text-brand-soft" />
          <AnimatePresence mode="wait">
            <motion.span
              key={stageLabel}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={transition.normal}
              className="text-sm text-foreground/60"
            >
              {stageLabel}
            </motion.span>
          </AnimatePresence>
        </div>
        <div className="mt-2 h-0.5 overflow-hidden rounded-full bg-white/[0.06]">
          <motion.div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
            initial={{ width: '0%' }}
            animate={{ width: '90%' }}
            transition={transition.fakeProgressSlow}
          />
        </div>
      </div>

      {/* Streaming preview */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        <div className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/25 mb-3">
          {activeOut === 'resume'
            ? t('aiGenerate.generatingResume')
            : t('aiGenerate.generatingCoverLetter')}
        </div>
        <pre className="whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-foreground/40">
          {streamBuffer}
          <span className="inline-block h-3 w-0.5 animate-pulse bg-brand ml-0.5 align-middle" />
        </pre>
      </div>
    </motion.div>
  );
}
