import {
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  ExternalLink,
  Globe,
  SkipForward,
  X,
} from 'lucide-react';
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
          <p className="text-sm text-foreground/70">Checking browser availability…</p>
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
            <h2 className="text-2xl font-semibold text-foreground">Browser Detected</h2>
            <p className="mt-2 text-sm text-foreground/70">
              {browserPath ? `Found at: ${browserPath}` : 'Chrome or Edge is available'}
            </p>
          </div>
          <p className="text-sm text-foreground/50">Proceeding to next step…</p>
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
          Back
        </button>
        <button
          onClick={onNext}
          className="flex items-center gap-2 text-sm text-foreground/50 transition-colors hover:text-foreground"
        >
          Skip
          <SkipForward className="h-4 w-4" />
        </button>
      </div>

      <div className="mb-8 flex flex-col items-center gap-4">
        <div className="flex h-16 w-16 items-center justify-center rounded-full bg-brand-soft">
          <Globe className="h-8 w-8 text-brand" />
        </div>
        <h2 className="text-2xl font-semibold text-foreground">Browser Required</h2>
        <p className="text-center text-sm text-foreground/70">
          Chrome or Edge is needed for job board login and automated job applications.
        </p>
      </div>

      <div className="mb-8 rounded-xl bg-destructive/10 p-4">
        <div className="flex items-start gap-3">
          <X className="mt-0.5 h-5 w-5 flex-shrink-0 text-destructive" />
          <div className="flex-1">
            <p className="text-sm font-medium text-destructive">No browser detected</p>
            <p className="mt-1 text-xs text-foreground/70">
              Without Chrome or Edge, the app will download a ~120 MB Chromium binary when you first
              try to connect to a job board. Installing Chrome is recommended for better
              performance.
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
          Download Chrome
        </Button>
        <Button
          onClick={openEdgeDownload}
          className="w-full"
          variant="ghost"
          disabled={openExternal.isPending}
        >
          <ExternalLink className="mr-2 h-4 w-4" />
          Download Edge
        </Button>
        <Button onClick={onNext} className="w-full" variant="ghost">
          <SkipForward className="mr-2 h-4 w-4" />
          Skip for now
        </Button>
      </div>

      <p className="mt-6 text-center text-xs text-foreground/50">
        You can also set the <code className="rounded bg-foreground/10 px-1 py-0.5">CHROME</code>{' '}
        environment variable to point to a custom browser installation.
      </p>
    </motion.div>
  );
}
