import { Info, Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  show: boolean;
  connectPending: boolean;
  onConnect: () => void;
}

export function AuthHint({ show, connectPending, onConnect }: Props) {
  const { t } = useTranslation();

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          key="auth-hint"
          initial={{ opacity: 0, height: 0, marginBottom: 0 }}
          animate={{ opacity: 1, height: 'auto', marginBottom: 16 }}
          exit={{ opacity: 0, height: 0, marginBottom: 0 }}
          transition={transition.fast}
          className="overflow-hidden"
        >
          <div className="flex items-center gap-2 rounded-lg border border-blue-400/15 bg-blue-400/5 px-3 py-2 text-[11px] text-blue-200/75">
            <Info size={12} className="shrink-0 text-blue-400/60" />
            <span>{t('jobs.authHint')}</span>
            <button
              type="button"
              disabled={connectPending}
              onClick={onConnect}
              className="ml-auto shrink-0 text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
            >
              {connectPending ? (
                <Loader2 size={11} className="animate-spin" />
              ) : (
                t('jobs.authHintLink')
              )}
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
