import { Globe } from 'lucide-react';
import { motion } from 'motion/react';

interface BrowserLoadingStateProps {
  message: string;
}

export function BrowserLoadingState({ message }: BrowserLoadingStateProps) {
  return (
    <div className="flex flex-col items-center gap-4 py-8">
      <motion.div
        animate={{ rotate: 360 }}
        transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}
        className="relative"
      >
        <div className="absolute inset-0 rounded-full bg-brand/20 blur-xl" />
        <div className="relative flex h-20 w-20 items-center justify-center rounded-2xl bg-brand/10 ring-1 ring-brand/30">
          <Globe size={32} className="text-brand-soft" />
        </div>
      </motion.div>
      <p className="text-sm text-foreground/50">{message}</p>
    </div>
  );
}
