import type { LucideIcon } from 'lucide-react';
import { motion } from 'motion/react';
import type { ReactNode } from 'react';

interface FloatingIconProps {
  icon: LucideIcon;
  size?: number;
  children?: ReactNode;
}

export function FloatingIcon({ icon: Icon, size = 24, children }: FloatingIconProps) {
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
            'linear-gradient(135deg, color-mix(in srgb, var(--color-brand) 25%, transparent) 0%, color-mix(in srgb, var(--color-brand-2) 15%, transparent) 100%)',
          border: '1px solid color-mix(in srgb, var(--color-brand) 30%, transparent)',
          boxShadow: '0 0 32px color-mix(in srgb, var(--color-brand) 20%, transparent)',
        }}
      >
        {children || <Icon size={size} className="text-brand-soft" />}
      </div>
    </motion.div>
  );
}
