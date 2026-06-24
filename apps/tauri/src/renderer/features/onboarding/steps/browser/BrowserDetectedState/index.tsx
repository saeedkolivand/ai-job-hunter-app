import { ArrowLeft, ArrowRight, ChevronDown, ChevronUp, Globe } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, transition, withDelay } from '@ajh/ui';

interface BrowserDetectedStateProps {
  browserPath: string;
  onBack: () => void;
  onNext: () => void;
}

/**
 * Derive a friendly browser label from the launch command / path returned by
 * `system_check_browser`. Covers native paths, Snap wrappers, and Flatpak
 * `flatpak run <app-id>` strings. Falls back to `null` so the caller can use a
 * generic label instead of silently showing the wrong brand.
 */
export function getBrowserLabel(browserPath: string): string | null {
  const lower = browserPath.toLowerCase();
  if (lower.includes('brave')) return 'Brave';
  if (lower.includes('vivaldi')) return 'Vivaldi';
  if (lower.includes('edge') || lower.includes('msedge')) return 'Edge';
  if (lower.includes('chromium')) return 'Chromium';
  if (lower.includes('chrome')) return 'Chrome';
  return null;
}

export function BrowserDetectedState({ browserPath, onBack, onNext }: BrowserDetectedStateProps) {
  const { t } = useTranslation();
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);

  const browserLabel = getBrowserLabel(browserPath);
  const displayName = browserLabel ?? t('onboarding.browser.detected');

  return (
    <div className="space-y-5">
      {/* Animated success icon */}
      <div className="mb-6 flex justify-center">
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={transition.spring}
          className="relative"
        >
          <motion.div
            animate={{
              boxShadow: ['0 0 0 0 rgba(52, 211, 153, 0.4)', '0 0 0 20px rgba(52, 211, 153, 0)'],
            }}
            transition={transition.ping}
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
          transition={withDelay(0.1)}
          className="text-2xl font-semibold text-foreground/95"
        >
          {t('onboarding.browser.ready')}
        </motion.h2>
        <motion.p
          initial={{ y: 10, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          transition={withDelay(0.1)}
          className="mt-2 text-sm text-foreground/50"
        >
          {t('onboarding.browser.willUse')}
        </motion.p>
      </div>

      {/* Browser info card */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="surface-card rounded-xl p-4"
      >
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-red-500 via-yellow-500 to-green-500">
            <span className="text-lg font-bold text-white">{displayName[0]}</span>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-foreground/90">{displayName}</p>
            <p className="text-xs text-foreground/40 truncate">
              {t('onboarding.browser.readyLabel')}
            </p>
          </div>
        </div>
      </motion.div>

      {/* Expandable technical details */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
      >
        <Button
          variant="default"
          onClick={() => setShowTechnicalDetails(!showTechnicalDetails)}
          className="h-auto w-full justify-between rounded-lg border-[var(--border-clear)] bg-card px-4 py-2.5 text-left font-normal hover:bg-muted"
        >
          <span className="text-xs text-foreground/40">View technical details</span>
          {showTechnicalDetails ? (
            <ChevronUp size={14} className="text-foreground/30" />
          ) : (
            <ChevronDown size={14} className="text-foreground/30" />
          )}
        </Button>
        <AnimatePresence>
          {showTechnicalDetails && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={transition.normal}
              className="overflow-hidden"
            >
              <div className="mt-2 rounded-lg border border-[var(--border-clear)] bg-black/30 p-3">
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
          onClick={onBack}
          className="text-foreground/40 hover:text-foreground/70"
        >
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button variant="primary" onClick={onNext} className="px-8">
          {t('onboarding.browser.next')}
          <ArrowRight size={15} className="ml-2" />
        </Button>
      </div>
    </div>
  );
}
