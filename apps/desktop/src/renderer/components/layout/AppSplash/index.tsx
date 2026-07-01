import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useReducer } from 'react';

import { useTranslation } from '@ajh/translations';
import { transition } from '@ajh/ui';

/** Minimum display duration ms — covers the initial render without blocking. */
const MIN_DISPLAY_MS = 700;

/**
 * Budget after dismiss for the exit animation to complete before we force-unmount.
 * Covers transition.slow (350 ms) with headroom for throttled WAAPI on background tabs.
 */
const EXIT_BUDGET_MS = 500;

/**
 * Full-viewport branded overlay that covers the initial render and fades out
 * after MIN_DISPLAY_MS. Respects prefers-reduced-motion: no shimmer, instant
 * hide. Mount once at the app root (main.tsx); no IPC dependency.
 *
 * Hardened against stalled exit animations (e.g. WAAPI throttled on backgrounded tabs):
 *   - pointer-events-none applied the moment visible flips false (mid-fade safe).
 *   - onAnimationComplete unmounts on exit completion (happy path).
 *   - Hard fallback setTimeout force-unmounts after EXIT_BUDGET_MS if the
 *     animation never reports completion.
 */
export function AppSplash() {
  const { t } = useTranslation();
  const [visible, dismiss] = useReducer(() => false, true);
  const [mounted, unmount] = useReducer(() => false, true);

  useEffect(() => {
    const dismissId = setTimeout(dismiss, MIN_DISPLAY_MS);
    return () => clearTimeout(dismissId);
  }, []);

  // Hard fallback: force-unmount after dismiss + EXIT_BUDGET_MS in case
  // onAnimationComplete never fires (e.g. WAAPI throttled on a backgrounded tab).
  useEffect(() => {
    if (visible) return;
    const fallbackId = setTimeout(unmount, EXIT_BUDGET_MS);
    return () => clearTimeout(fallbackId);
  }, [visible]);

  const reducedMotion =
    typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  if (!mounted) return null;

  return (
    // pointer-events-none is applied to the shell the moment visible flips false
    // so mid-fade the overlay never intercepts clicks even if WAAPI is throttled.
    <div className={visible ? undefined : 'pointer-events-none'}>
      <AnimatePresence>
        {visible && (
          <motion.div
            key="app-splash"
            role="status"
            aria-label={t('app.title')}
            aria-live="polite"
            className="fixed inset-0 z-[9999] flex flex-col items-center justify-center bg-background"
            initial={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={reducedMotion ? transition.instant : transition.slow}
            onAnimationComplete={unmount}
          >
            {/* Brand wordmark — "AI Job Hunter" is a proper noun / brand name */}
            <h1 className="text-gradient select-none text-4xl font-bold tracking-tight">
              {t('app.title')}
            </h1>

            {/* Tagline */}
            <p className="mt-2 select-none text-sm text-foreground/50">{t('app.tagline')}</p>

            {/* Indeterminate shimmer bar — hidden for reduced-motion users */}
            {!reducedMotion && (
              <div
                className="mt-8 h-0.5 w-48 overflow-hidden rounded-full bg-foreground/10"
                aria-hidden="true"
              >
                <div className="bg-brand-gradient h-full w-full animate-pulse" />
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
