import { Download, Loader2, Sparkles, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { Button, RefreshButton } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@ajh/ui';
import { useUpdater } from '@/services/use-updater';

export function UpdateBanner() {
  const {
    status,
    download,
    install,
    downloadSpeed,
    downloadedBytes,
    totalBytes,
    timeRemaining,
    formatBytes,
  } = useUpdater();
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
            ) : (
              <Sparkles size={14} className="shrink-0 text-brand-soft" />
            )}

            {/* Message */}
            <div className="flex flex-col">
              <span className="text-xs text-foreground/85">
                {status.state === 'available' &&
                  t('updater.available', { version: status.version })}
                {status.state === 'downloading' &&
                  t('updater.downloading', { percent: status.percent })}
                {status.state === 'downloaded' &&
                  t('updater.downloaded', { version: status.version })}
              </span>
              {status.state === 'downloading' && (
                <div className="flex items-center gap-3 text-[10px] text-foreground/50">
                  {downloadSpeed && <span>{downloadSpeed}</span>}
                  {totalBytes > 0 && (
                    <span>
                      {formatBytes(downloadedBytes)}/{formatBytes(totalBytes)}
                    </span>
                  )}
                  {timeRemaining && <span>{timeRemaining}</span>}
                </div>
              )}
            </div>

            {/* Action button */}
            {status.state === 'available' && (
              <Button
                onClick={() => void download()}
                className="flex items-center gap-1.5 rounded-lg border border-brand/40 bg-brand/20 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/30"
              >
                <Download size={11} />
                {t('updater.downloadButton')}
              </Button>
            )}
            {status.state === 'downloaded' && (
              <RefreshButton
                onRefresh={install}
                className="flex items-center gap-1.5 rounded-lg border border-brand/40 bg-brand/20 px-2.5 py-1 text-[11px] font-medium text-brand-soft transition-colors hover:bg-brand/30"
              >
                {t('updater.installButton')}
              </RefreshButton>
            )}

            {/* Dismiss */}
            {status.state !== 'downloading' && (
              <Button
                onClick={() => setDismissed(true)}
                className="ml-1 text-foreground/30 transition-colors hover:text-foreground/60"
              >
                <X size={13} />
              </Button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
