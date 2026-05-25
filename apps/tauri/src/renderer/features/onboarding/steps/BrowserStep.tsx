import { ArrowLeft, CheckCircle2, ExternalLink, Globe, SkipForward, X } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect } from 'react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useCheckBrowser, useOpenExternal } from '@/services';

interface BrowserStepProps {
  direction: number;
  onBack: () => void;
  onNext: () => void;
}

export function BrowserStep({ direction, onBack, onNext }: BrowserStepProps) {
  const { t } = useTranslation();
  const { data: browserCheck, isLoading } = useCheckBrowser();
  const openExternal = useOpenExternal();

  const isDetected = browserCheck?.detected ?? false;
  const browserPath = browserCheck?.path;

  const openChromeDownload = () => {
    openExternal.mutate('https://www.google.com/chrome/');
  };

  const openEdgeDownload = () => {
    openExternal.mutate('https://www.microsoft.com/edge');
  };

  useEffect(() => {
    // Auto-advance if browser is already detected
    if (isDetected && !isLoading) {
      const timer = setTimeout(() => {
        onNext();
      }, 1500);
      return () => clearTimeout(timer);
    }
  }, [isDetected, isLoading, onNext]);

  if (isLoading) {
    return (
      <motion.div
        key="browser"
        initial={{ opacity: 0, x: 50 * direction }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: -50 * direction }}
        transition={transition.normal}
        className="w-full max-w-2xl rounded-2xl bg-surface p-8 shadow-2xl"
      >
        <div className="flex flex-col items-center justify-center gap-4 py-12">
          <div className="h-12 w-12 animate-spin rounded-full border-4 border-brand border-t-transparent" />
          <p className="text-sm text-foreground/70">{t('onboarding.browser.checking')}</p>
        </div>
      </motion.div>
    );
  }

  if (isDetected) {
    return (
      <motion.div
        key="browser"
        initial={{ opacity: 0, x: 50 * direction }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: -50 * direction }}
        transition={transition.normal}
        className="w-full max-w-2xl rounded-2xl bg-surface p-8 shadow-2xl"
      >
        <div className="flex flex-col items-center justify-center gap-6 py-8">
          <div className="flex h-20 w-20 items-center justify-center rounded-full bg-green-500/20">
            <CheckCircle2 className="h-10 w-10 text-green-400" />
          </div>
          <div className="text-center">
            <h2 className="text-2xl font-semibold text-foreground">
              {t('onboarding.browser.detected')}
            </h2>
            <p className="mt-2 text-sm text-foreground/70">
              {browserPath
                ? t('onboarding.browser.foundAt', { path: browserPath })
                : t('onboarding.browser.available')}
            </p>
          </div>
          <p className="text-sm text-foreground/50">{t('onboarding.browser.proceeding')}</p>
        </div>
      </motion.div>
    );
  }

  return (
    <motion.div
      key="browser"
      initial={{ opacity: 0, x: 50 * direction }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -50 * direction }}
      transition={transition.normal}
      className="w-full max-w-2xl rounded-2xl bg-surface p-8 shadow-2xl"
    >
      <div className="mb-6 flex items-center justify-between">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-sm text-foreground/50 transition-colors hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" />
          {t('onboarding.back')}
        </button>
        <button
          onClick={onNext}
          className="flex items-center gap-2 text-sm text-foreground/50 transition-colors hover:text-foreground"
        >
          {t('onboarding.skip')}
          <SkipForward className="h-4 w-4" />
        </button>
      </div>

      <div className="mb-8 flex flex-col items-center gap-4">
        <div className="flex h-16 w-16 items-center justify-center rounded-full bg-brand-soft">
          <Globe className="h-8 w-8 text-brand" />
        </div>
        <h2 className="text-2xl font-semibold text-foreground">{t('onboarding.browser.title')}</h2>
        <p className="text-center text-sm text-foreground/70">{t('onboarding.browser.subtitle')}</p>
      </div>

      <div className="mb-8 rounded-xl bg-destructive/10 p-4">
        <div className="flex items-start gap-3">
          <X className="mt-0.5 h-5 w-5 flex-shrink-0 text-destructive" />
          <div className="flex-1">
            <p className="text-sm font-medium text-destructive">
              {t('onboarding.browser.notDetected')}
            </p>
            <p className="mt-1 text-xs text-foreground/70">
              {t('onboarding.browser.notDetectedDesc')}
            </p>
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-3">
        <Button
          onClick={openChromeDownload}
          className="w-full"
          variant="default"
          disabled={openExternal.isPending}
        >
          <ExternalLink className="mr-2 h-4 w-4" />
          {t('onboarding.browser.downloadChrome')}
        </Button>
        <Button
          onClick={openEdgeDownload}
          className="w-full"
          variant="ghost"
          disabled={openExternal.isPending}
        >
          <ExternalLink className="mr-2 h-4 w-4" />
          {t('onboarding.browser.downloadEdge')}
        </Button>
        <Button onClick={onNext} className="w-full" variant="ghost">
          <SkipForward className="mr-2 h-4 w-4" />
          {t('onboarding.browser.skip')}
        </Button>
      </div>

      <p className="mt-6 text-center text-xs text-foreground/50">
        {t('onboarding.browser.envVar')}
      </p>
    </motion.div>
  );
}
