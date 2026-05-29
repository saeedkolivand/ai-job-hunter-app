import { motion } from 'motion/react';

import { cn } from '@ajh/ui';

import type { ApplyStep } from './types';

export function StepRow({ step }: { step: ApplyStep }) {
  if (step.kind === 'progress') {
    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="rounded-md bg-white/[0.02] px-2.5 py-1.5"
      >
        <div className="mb-1 flex items-center justify-between text-[11px]">
          <span className="text-foreground/65">{step.stage}</span>
          <span className="text-foreground/45">{Math.round((step.p ?? 0) * 100)}%</span>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-white/5">
          <div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
            style={{ width: `${(step.p ?? 0) * 100}%` }}
          />
        </div>
      </motion.div>
    );
  }
  return (
    <motion.div
      initial={{ opacity: 0, x: 6 }}
      animate={{ opacity: 1, x: 0 }}
      className="flex items-start gap-2 text-[12px]"
    >
      <span
        className={cn(
          'mt-1.5 h-1.5 w-1.5 flex-shrink-0 rounded-full shadow-[0_0_8px_currentColor]',
          step.ok ? 'bg-emerald-400' : 'bg-amber-400'
        )}
      />
      <div className="flex-1">
        <span className="text-foreground/85">{step.stage}</span>
        {step.note && <div className="text-[11px] text-foreground/45">{step.note}</div>}
      </div>
    </motion.div>
  );
}
