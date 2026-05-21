import { motion } from 'motion/react';

import { transition, variants } from '@/lib/motion';

interface PageTransitionProps {
  children: React.ReactNode;
  className?: string;
}

export function PageTransition({ children, className }: PageTransitionProps) {
  return (
    <motion.div
      {...variants.pageIn}
      transition={transition.relaxed}
      className={className ?? 'h-full'}
    >
      {children}
    </motion.div>
  );
}
