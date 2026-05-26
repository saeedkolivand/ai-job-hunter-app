import { Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useState } from 'react';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';

import { ThinkingBubble } from './ThinkingBubble';

interface OutputPanelGeneratingProps {
  stageLabel: string;
  streamBuffer: string;
  activeOut: 'resume' | 'cover';
  thinkingBuffer?: string;
  wordCount?: number;
}

export function OutputPanelGenerating({
  stageLabel,
  streamBuffer,
  activeOut,
  thinkingBuffer = '',
  wordCount = 0,
}: OutputPanelGeneratingProps) {
  const { t } = useTranslation();
  const [elapsed, setElapsed] = useState(0);
  const hasOutput = streamBuffer.length > 0;

  // Tick elapsed seconds while waiting for first output token
  useEffect(() => {
    if (hasOutput) {
      setElapsed(0);
      return;
    }
    setElapsed(0);
    const id = setInterval(() => setElapsed((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, [hasOutput]);

  return (
    <motion.div
      key="generating"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Header */}
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
              className="flex-1 text-sm text-foreground/60"
            >
              {stageLabel}
            </motion.span>
          </AnimatePresence>
          {wordCount > 0 && (
            <span className="shrink-0 tabular-nums text-[10px] text-foreground/30">
              {wordCount} words
            </span>
          )}
        </div>
        <div className="mt-2 h-0.5 overflow-hidden rounded-full bg-white/[0.06]">
          <motion.div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
            initial={{ width: '0%' }}
            animate={{ width: hasOutput ? '90%' : '15%' }}
            transition={
              hasOutput
                ? transition.fakeProgressSlow
                : { duration: 1.8, repeat: Infinity, repeatType: 'reverse', ease: 'easeInOut' }
            }
          />
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        <ThinkingBubble thinking={thinkingBuffer} done={hasOutput} />

        <AnimatePresence mode="wait">
          {!hasOutput ? (
            <motion.div
              key="waiting"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={transition.normal}
              className="flex flex-col items-center justify-center gap-3 py-10 text-center"
            >
              <div className="flex gap-1.5">
                {[0, 200, 400].map((delay) => (
                  <motion.span
                    key={delay}
                    className="h-2 w-2 rounded-full bg-brand/40"
                    animate={{ scale: [1, 1.4, 1], opacity: [0.4, 1, 0.4] }}
                    transition={{
                      duration: 1.2,
                      repeat: Infinity,
                      delay: delay / 1000,
                      ease: 'easeInOut',
                    }}
                  />
                ))}
              </div>
              <p className="text-xs text-foreground/40">
                {thinkingBuffer.length > 0
                  ? 'Model is reasoning, output coming soon…'
                  : 'Waiting for model response…'}
              </p>
              {elapsed >= 5 && (
                <motion.p
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  className="tabular-nums text-[10px] text-foreground/25"
                >
                  {elapsed}s — local models may take a moment
                </motion.p>
              )}
            </motion.div>
          ) : (
            <motion.div
              key="streaming"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={transition.normal}
            >
              <div className="mb-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/25">
                {activeOut === 'resume'
                  ? t('aiGenerate.generatingResume')
                  : t('aiGenerate.generatingCoverLetter')}
              </div>
              <pre className="whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-foreground/40">
                {streamBuffer}
                <span className="ml-0.5 inline-block h-3 w-0.5 animate-pulse bg-brand align-middle" />
              </pre>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
