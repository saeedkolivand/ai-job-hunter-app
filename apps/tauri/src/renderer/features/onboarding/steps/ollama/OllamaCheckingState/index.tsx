import { Loader2 } from 'lucide-react';
import { motion } from 'motion/react';

import { transition } from '@ajh/ui';

interface OllamaCheckingStateProps {
  message: string;
}

export function OllamaCheckingState({ message }: OllamaCheckingStateProps) {
  return (
    <motion.div
      key="local-checking"
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -20, scale: 0.95 }}
      transition={transition.slow}
      className="mb-6 flex flex-col items-center gap-3 py-6"
    >
      <Loader2 size={24} className="animate-spin text-brand-soft" />
      <p className="text-sm text-foreground/40">{message}</p>
    </motion.div>
  );
}
