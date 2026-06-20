import { Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import type { BoardAuthRequirement } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, transition } from '@ajh/ui';

interface Props {
  show: boolean;
  board: string;
  auth: BoardAuthRequirement;
  boardConnected: boolean;
  disconnectPending: boolean;
  connectPending: boolean;
  onDisconnect: () => void;
  onConnect: () => void;
}

export function AuthModeBadge({
  show,
  board,
  auth,
  boardConnected,
  disconnectPending,
  connectPending,
  onDisconnect,
  onConnect,
}: Props) {
  const { t } = useTranslation();

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          key={`mode-${board}-${boardConnected ? 'auth' : auth}`}
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
              <Button
                variant="unstyled"
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
              </Button>
            </>
          ) : auth === 'required' ? (
            <>
              <span className="inline-flex items-center gap-1 rounded-full bg-red-500/15 px-2 py-0.5 text-[10px] font-medium text-red-400">
                <span className="h-1.5 w-1.5 rounded-full bg-red-400" />
                {t('jobs.modeLoginRequired')}
              </span>
              <span className="text-[10px] text-foreground/35">
                {t('jobs.modeLoginRequiredNote')}
              </span>
              <Button
                variant="unstyled"
                type="button"
                disabled={connectPending}
                onClick={onConnect}
                className="ml-auto shrink-0 text-[10px] text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
              >
                {connectPending ? <Loader2 size={10} className="animate-spin" /> : t('jobs.logIn')}
              </Button>
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
