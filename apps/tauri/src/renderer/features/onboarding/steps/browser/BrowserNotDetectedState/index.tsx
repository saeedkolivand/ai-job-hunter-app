import { ArrowLeft, ArrowRight, ExternalLink, Globe, X } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, transition, withDelay } from '@ajh/ui';

import { useOpenExternal } from '@/services';

interface BrowserNotDetectedStateProps {
  onBack: () => void;
  onNext: () => void;
}

export function BrowserNotDetectedState({ onBack, onNext }: BrowserNotDetectedStateProps) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();

  const openChromeDownload = () => {
    openExternal.mutate('https://www.google.com/chrome/');
  };

  const openEdgeDownload = () => {
    openExternal.mutate('https://www.microsoft.com/edge');
  };

  return (
    <motion.div
      initial={{ y: 10, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={withDelay(0.1)}
      className="space-y-5"
    >
      {/* Icon */}
      <div className="mb-6 flex justify-center">
        <motion.div
          animate={{
            y: [0, -8, 0],
          }}
          transition={transition.breathe}
          className="relative"
        >
          <div className="absolute inset-0 rounded-full bg-brand/20 blur-xl" />
          <div
            className="relative flex h-16 w-16 items-center justify-center rounded-2xl"
            style={{
              background:
                'linear-gradient(135deg, color-mix(in srgb, var(--color-brand) 25%, transparent) 0%, color-mix(in srgb, var(--color-brand-2) 15%, transparent) 100%)',
              border: '1px solid color-mix(in srgb, var(--color-brand) 30%, transparent)',
              boxShadow: '0 0 32px color-mix(in srgb, var(--color-brand) 20%, transparent)',
            }}
          >
            <Globe size={28} className="text-brand-soft" />
          </div>
        </motion.div>
      </div>

      {/* Heading */}
      <div className="text-center">
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.browser.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.browser.subtitle')}</p>
      </div>

      {/* Not detected warning */}
      <div className="rounded-xl border border-amber-400/20 bg-amber-400/5 p-4">
        <div className="flex items-start gap-3">
          <X className="mt-0.5 h-5 w-5 flex-shrink-0 text-amber-400" />
          <div className="flex-1">
            <p className="text-sm font-medium text-amber-200">
              {t('onboarding.browser.notDetected')}
            </p>
            <p className="mt-1 text-xs text-amber-200/60">
              {t('onboarding.browser.notDetectedDesc')}
            </p>
          </div>
        </div>
      </div>

      {/* Download buttons */}
      <div className="space-y-2">
        <Button
          onClick={openChromeDownload}
          className="w-full justify-center gap-2"
          variant="default"
          disabled={openExternal.isPending}
        >
          <ExternalLink size={14} />
          {t('onboarding.browser.downloadChrome')}
        </Button>
        <Button
          onClick={openEdgeDownload}
          className="w-full justify-center gap-2"
          variant="ghost"
          disabled={openExternal.isPending}
        >
          <ExternalLink size={14} />
          {t('onboarding.browser.downloadEdge')}
        </Button>
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between pt-2">
        <Button
          variant="ghost"
          onClick={onBack}
          className="text-foreground/40 hover:text-foreground/70"
        >
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button
          variant="glass"
          size="md"
          onClick={onNext}
          className="transition-all duration-150 ease-out px-6 gap-2"
        >
          {t('onboarding.skip')}
          <ArrowRight size={14} />
        </Button>
      </div>

      <p className="text-center text-xs text-foreground/35">{t('onboarding.browser.envVar')}</p>
    </motion.div>
  );
}
