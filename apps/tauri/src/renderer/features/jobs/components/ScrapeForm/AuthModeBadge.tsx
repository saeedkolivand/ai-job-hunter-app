import { Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  show: boolean;
  board: string;
  boardConnected: boolean;
  disconnectPending: boolean;
  onDisconnect: () => void;
}

export function AuthModeBadge({
  show,
  board,
  boardConnected,
  disconnectPending,
  onDisconnect,
}: Props) {
  const { t } = useTranslation();

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          key={`mode-${board}-${boardConnected ? 'auth' : 'guest'}`}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={transition.fast}
          className="mb-3 flex items-center gap-1.5"
        >
          {boardConnected ? (
            <>
              <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                {t('jobs.modeAuthenticated')}
              </span>
              <span className="text-[10px] text-foreground/35">{t('jobs.modeAuthNote')}</span>
              <button
                type="button"
                disabled={disconnectPending}
                onClick={onDisconnect}
                className="ml-auto shrink-0 text-[10px] text-red-400/70 underline-offset-2 hover:text-red-400 hover:underline disabled:opacity-50"
              >
                {disconnectPending ? (
                  <Loader2 size={10} className="animate-spin" />
                ) : (
                  t('jobs.disconnect')
                )}
              </button>
            </>
          ) : (
            <>
              <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] font-medium text-amber-400">
                <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                {t('jobs.modeGuest')}
              </span>
              <span className="text-[10px] text-foreground/35">{t('jobs.modeGuestNote')}</span>
            </>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
