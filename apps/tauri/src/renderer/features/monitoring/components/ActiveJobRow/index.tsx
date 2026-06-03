import { Loader2 } from 'lucide-react';
import { motion } from 'motion/react';

import { cn, transition } from '@ajh/ui';

import type { JobRecord } from '@/features/monitoring/types';

const KIND_SHORT: Record<string, string> = {
  'ai.generate': 'AI',
  'ai.embed': 'Embed',
  'document.import': 'Doc',
  'scrape.board': 'Scrape',
  'scrape.url': 'Scrape',
  'persist.job': 'Save',
  'match.resume': 'Match',
  'autopilot.run': 'Autopilot',
};

interface Props {
  job: JobRecord;
  kindLabel: Record<string, string>;
  t: (key: string) => string;
}

export function ActiveJobRow({ job, kindLabel, t }: Props) {
  const _label = KIND_SHORT[job.kind] ?? job.kind;
  const isStreaming = job.status === 'streaming';

  return (
    <motion.div
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 4 }}
      transition={transition.normal}
      className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2"
    >
      <div className="flex items-center justify-between gap-2 mb-1.5">
        <div className="flex items-center gap-1.5">
          <Loader2 size={10} className="animate-spin text-brand-soft" />
          <span className="text-xs font-medium text-foreground/80">
            {kindLabel[job.kind] ?? job.kind}
          </span>
        </div>
        <span
          className={cn(
            'text-[10px] font-medium',
            isStreaming ? 'text-blue-400' : 'text-foreground/40'
          )}
        >
          {isStreaming ? t('monitoring.timeLabels.streaming') : job.status}
        </span>
      </div>
      {job.progress > 0 && (
        <div className="h-0.5 w-full overflow-hidden rounded-full bg-white/[0.06]">
          <div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary transition-all duration-300"
            style={{ width: `${Math.round(job.progress * 100)}%` }}
          />
        </div>
      )}
      <div className="mt-1 text-[10px] text-foreground/30 font-mono">{job.id.slice(0, 12)}…</div>
    </motion.div>
  );
}
