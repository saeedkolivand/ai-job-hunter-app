import { motion } from 'motion/react';

import { cn } from '../../lib/cn';
import { transition } from '../../lib/motion';

export interface ProgressBarProps {
  /** Current progress value (0-100) */
  value: number;
  /** Height of the progress bar in pixels */
  height?: number;
  /** Additional class names */
  className?: string;
  /** Whether to show percentage label */
  showLabel?: boolean;
  /** Custom gradient colors */
  gradientFrom?: string;
  gradientVia?: string;
  gradientTo?: string;
  /** Label position: 'end' or 'start' */
  labelPosition?: 'start' | 'end';
}

export function ProgressBar({
  value,
  height = 6,
  className: _className,
  showLabel = true,
  gradientFrom = 'from-violet-700',
  gradientVia = 'via-brand',
  gradientTo = 'to-brand-soft',
  labelPosition = 'end',
}: ProgressBarProps) {
  const clampedValue = Math.min(Math.max(value, 0), 100);

  return (
    <div className="space-y-1.5">
      <div className={`h-${height / 4} overflow-hidden rounded-full bg-white/[0.06]`}>
        <motion.div
          className={cn(
            'h-full rounded-full bg-gradient-to-r',
            gradientFrom,
            gradientVia,
            gradientTo
          )}
          animate={{ width: `${clampedValue}%` }}
          transition={transition.progress}
        />
      </div>
      {showLabel && (
        <div
          className={cn(
            'flex text-[10px] text-foreground/20',
            labelPosition === 'end' ? 'justify-end' : 'justify-start'
          )}
        >
          <span>{Math.round(clampedValue)}%</span>
        </div>
      )}
    </div>
  );
}
