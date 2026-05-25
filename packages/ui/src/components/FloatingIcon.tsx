import { LucideIcon } from 'lucide-react';
import { motion } from 'motion/react';
import { ReactNode } from 'react';

interface FloatingIconProps {
  icon: LucideIcon;
  size?: number;
  className?: string;
  children?: ReactNode;
}

export function FloatingIcon({ icon: Icon, size = 24, className, children }: FloatingIconProps) {
  return (
    <motion.div
      animate={{
        y: [0, -8, 0],
      }}
      transition={{ duration: 3, repeat: Infinity, ease: 'easeInOut' }}
      className="relative"
    >
      <div className="absolute inset-0 rounded-full bg-brand/20 blur-xl" />
      <div
        className="relative flex h-14 w-14 items-center justify-center rounded-2xl"
        style={{
          background:
            'linear-gradient(135deg, rgba(168,85,247,0.25) 0%, rgba(99,102,241,0.15) 100%)',
          border: '1px solid rgba(168,85,247,0.3)',
          boxShadow: '0 0 32px rgba(168,85,247,0.2)',
        }}
      >
        {children || <Icon size={size} className="text-brand-soft" />}
      </div>
    </motion.div>
  );
}
