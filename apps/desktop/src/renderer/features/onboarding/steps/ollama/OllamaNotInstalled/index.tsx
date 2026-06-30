import { ExternalLink, Loader2 } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Alert, Button, transition } from '@ajh/ui';

import { useOpenExternal } from '@/services';

interface OllamaNotInstalledProps {
  onRecheck: () => void;
}

export function OllamaNotInstalled({ onRecheck }: OllamaNotInstalledProps) {
  const { t } = useTranslation();
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
      <Alert
        type="warning"
        showIcon
        message={t('onboarding.ai.notFound')}
        description={t('onboarding.ai.notFoundDesc')}
      />

      <div className="space-y-2">
        <p className="text-xs font-semibold uppercase tracking-widest text-foreground/55">
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
        className="w-full justify-center gap-2"
        onClick={() => void openExternal.mutateAsync('https://ollama.com')}
      >
        <ExternalLink size={13} />
        Download Ollama
      </Button>

      <Button
        variant="ghost"
        className="w-full justify-center gap-1.5 text-foreground/40 hover:text-foreground/70"
        onClick={onRecheck}
      >
        <Loader2 size={12} />
        Recheck
      </Button>
    </motion.div>
  );
}
