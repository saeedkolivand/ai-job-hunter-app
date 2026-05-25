import { ArrowLeft, ArrowRight, ChevronDown, ChevronUp, Globe } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface BrowserDetectedStateProps {
  browserPath: string;
  onBack: () => void;
  onNext: () => void;
}

export function BrowserDetectedState({ browserPath, onBack, onNext }: BrowserDetectedStateProps) {
  const { t } = useTranslation();
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);

  return (
    <div className="space-y-5">
      {/* Animated success icon */}
      <div className="mb-6 flex justify-center">
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ duration: 0.5, type: 'spring' }}
          className="relative"
        >
          <motion.div
            animate={{
              boxShadow: ['0 0 0 0 rgba(52, 211, 153, 0.4)', '0 0 0 20px rgba(52, 211, 153, 0)'],
            }}
            transition={{ duration: 2, repeat: Infinity }}
            className="absolute inset-0 rounded-full bg-emerald-500/20 blur-xl"
          />
          <div className="relative flex h-20 w-20 items-center justify-center rounded-2xl bg-emerald-500/10 ring-1 ring-emerald-500/30">
            <Globe size={36} className="text-emerald-400" />
          </div>
        </motion.div>
      </div>

      {/* Success message */}
      <div className="text-center">
        <motion.h2
          initial={{ y: 10, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          transition={{ delay: 0.1 }}
          className="text-2xl font-semibold text-foreground/95"
        >
          Chrome Ready
        </motion.h2>
        <motion.p
          initial={{ y: 10, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          transition={{ delay: 0.1 }}
          className="mt-2 text-sm text-foreground/50"
        >
          We'll use Chrome for secure LinkedIn authentication
        </motion.p>
      </div>

      {/* Browser info card */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.2 }}
        className="rounded-xl border border-white/[0.08] bg-white/[0.02] p-4"
      >
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-red-500 via-yellow-500 to-green-500">
            <span className="text-lg font-bold text-white">C</span>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-foreground/90">Google Chrome</p>
            <p className="text-xs text-foreground/40 truncate">Detected and ready</p>
          </div>
        </div>
      </motion.div>

      {/* Expandable technical details */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.15 }}
      >
        <button
          onClick={() => setShowTechnicalDetails(!showTechnicalDetails)}
          className="flex w-full items-center justify-between rounded-lg border border-white/[0.06] bg-white/[0.02] px-4 py-2.5 text-left transition-all hover:border-white/10 hover:bg-white/[0.04]"
        >
          <span className="text-xs text-foreground/40">View technical details</span>
          {showTechnicalDetails ? (
            <ChevronUp size={14} className="text-foreground/30" />
          ) : (
            <ChevronDown size={14} className="text-foreground/30" />
          )}
        </button>
        <AnimatePresence>
          {showTechnicalDetails && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden"
            >
              <div className="mt-2 rounded-lg border border-white/[0.06] bg-black/30 p-3">
                <p className="text-xs font-mono text-foreground/30 break-all">{browserPath}</p>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>

      {/* Footer */}
      <div className="flex items-center justify-between pt-4">
        <Button
          variant="ghost"
          size="sm"
          onClick={onBack}
          className="text-foreground/40 hover:text-foreground/70"
        >
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button onClick={onNext} className="px-8">
          {t('onboarding.continue')}
          <ArrowRight size={15} className="ml-2" />
        </Button>
      </div>
    </div>
  );
}
