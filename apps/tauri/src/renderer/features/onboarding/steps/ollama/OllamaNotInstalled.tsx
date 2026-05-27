import { ExternalLink, Loader2, WifiOff } from 'lucide-react';
import { motion } from 'motion/react';

import { Button } from '@ajh/ui';

import { transition } from '@ajh/ui';
import { useOpenExternal } from '@/services';

interface OllamaNotInstalledProps {
  onRecheck: () => void;
}

export function OllamaNotInstalled({ onRecheck }: OllamaNotInstalledProps) {
  const openExternal = useOpenExternal();

  return (
    <motion.div
      key="local-not-installed"
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -20, scale: 0.95 }}
      transition={transition.normal}
      className="mb-6 space-y-4"
    >
      <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/5 p-4">
        <WifiOff size={16} className="mt-0.5 shrink-0 text-amber-400" />
        <div>
          <p className="text-sm font-medium text-amber-200">Ollama not found</p>
          <p className="mt-1 text-xs text-amber-200/60">
            We couldn't detect Ollama on your system. Install it to use local AI models.
          </p>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-xs font-medium uppercase tracking-widest text-foreground/30">
          Installation steps
        </p>
        {['Download Ollama', 'Install the app', 'Run Ollama'].map((step, i) => (
          <div key={step} className="flex items-center gap-3 text-sm text-foreground/60">
            <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-white/10 text-[10px] text-foreground/30">
              {i + 1}
            </span>
            {step}
          </div>
        ))}
      </div>

      <Button
        variant="glass"
        size="sm"
        className="w-full justify-center gap-2"
        onClick={() => void openExternal.mutateAsync('https://ollama.com')}
      >
        <ExternalLink size={13} />
        Download Ollama
      </Button>

      <Button
        variant="ghost"
        size="sm"
        className="w-full justify-center gap-1.5 text-foreground/40 hover:text-foreground/70"
        onClick={onRecheck}
      >
        <Loader2 size={12} />
        Recheck
      </Button>
    </motion.div>
  );
}
