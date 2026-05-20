import { AnimatePresence, motion } from 'motion/react';
import { Download, RefreshCw, X, Sparkles, Loader2 } from 'lucide-react';
import { useState } from 'react';
import { useUpdater } from '@/services/use-updater';
import { transition } from '@/lib/motion';
import { useTranslation } from '@/lib/i18n';

export function UpdateBanner() {
  const { status, download, install } = useUpdater();
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState(false);

  const visible =
    !dismissed &&
    (status.state === 'available' ||
      status.state === 'downloading' ||
      status.state === 'downloaded');

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0, y: -40 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -40 }}
          transition={transition.normal}
          className="fixed left-1/2 top-3 z-[300] -translate-x-1/2"
        >
          <div
            className="flex items-center gap-3 rounded-xl border border-brand/30 px-4 py-2.5 shadow-2xl shadow-black/40"
            style={{
              background:
                'linear-gradient(135deg, rgba(168,85,247,0.15) 0%, rgba(99,102,241,0.10) 100%)',
              backdropFilter: 'blur(16px)',
            }}
          >
            {/* Icon */}
            {status.state === 'downloading' ? (
              <Loader2 size={14} className="shrink-0 animate-spin text-brand-soft" />
            ) : status.state === 'downloaded' ? (
              <RefreshCw size={14} className="shrink-0 text-brand-soft" />
            ) : (
              <Sparkles size={14} className="shrink-0 text-brand-soft" />
            )}

            {/* Message */}
            <span className="text-xs text-foreground/85">
              {status.state === 'available' && t('updater.available', { version: status.version })}
              {status.state === 'downloading' &&
                t('updater.downloading', { percent: status.percent })}
              {status.state === 'downloaded' &&
                t('updater.downloaded', { version: status.version })}
            </span>

            {/* Action button */}
            {status.state === 'available' && (
              <button
                onClick={() => void download()}
                className="flex items-center gap-1.5 rounded-lg border border-brand/40 bg-brand/20 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/30"
              >
                <Download size={11} />
                {t('updater.downloadButton')}
              </button>
            )}
            {status.state === 'downloaded' && (
              <button
                onClick={() => void install()}
                className="flex items-center gap-1.5 rounded-lg border border-brand/40 bg-brand/20 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/30"
              >
                <RefreshCw size={11} />
                {t('updater.installButton')}
              </button>
            )}

            {/* Dismiss */}
            {status.state !== 'downloading' && (
              <button
                onClick={() => setDismissed(true)}
                className="ml-1 text-foreground/30 transition-colors hover:text-foreground/60"
              >
                <X size={13} />
              </button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
