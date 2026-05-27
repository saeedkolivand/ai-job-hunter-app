import { Clock } from 'lucide-react';
import { motion } from 'motion/react';

import { cn, transition } from '@ajh/ui';

import { useAgo } from '../hooks/useAgo';
import type { ActivityItem } from '../types';

interface Props {
  a: ActivityItem;
}

export function ActivityRow({ a }: Props) {
  const ago = useAgo(a.time);
  const dotClass = {
    violet: 'bg-violet-400',
    indigo: 'bg-indigo-400',
    blue: 'bg-blue-400',
    emerald: 'bg-emerald-400',
    amber: 'bg-amber-400',
  }[a.tone];

  return (
    <motion.div
      initial={{ opacity: 0, x: 6 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0 }}
      transition={transition.fast}
      className="flex items-start gap-2.5 rounded-lg px-2 py-1.5 hover:bg-white/[0.02] transition-colors"
    >
      <span
        className={cn(
          'mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full shadow-[0_0_6px_currentColor]',
          dotClass
        )}
      />
      <div className="flex-1 text-[12px] text-foreground/70">{a.text}</div>
      <div className="flex shrink-0 items-center gap-0.5 text-[10px] text-foreground/30">
        <Clock size={9} />
        {ago}
      </div>
    </motion.div>
  );
}
