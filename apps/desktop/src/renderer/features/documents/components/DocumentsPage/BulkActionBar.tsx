import { Trash2 } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, transition } from '@ajh/ui';

interface BulkActionBarProps {
  count: number;
  deleting: boolean;
  onClear: () => void;
  onDelete: () => void;
}

/** Bulk-action bar — shown by the page (inside `AnimatePresence`) while ≥1 generation is selected. */
export function BulkActionBar({ count, deleting, onClear, onDelete }: BulkActionBarProps) {
  const { t } = useTranslation();
  return (
    <motion.div
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -6 }}
      transition={transition.fast}
      className="mb-4 flex items-center justify-between rounded-xl border border-red-400/20 bg-red-400/[0.06] px-4 py-2.5"
    >
      <span className="text-xs text-foreground/60">{t('resumes.select.count', { count })}</span>
      <div className="flex items-center gap-2">
        <Button
          onClick={onClear}
          className="h-auto rounded-lg border-transparent bg-transparent px-3 py-1.5 text-xs text-foreground/50 hover:text-foreground"
        >
          {t('resumes.select.clear')}
        </Button>
        <Button
          onClick={onDelete}
          disabled={deleting}
          className="flex h-auto items-center gap-1.5 rounded-lg border-red-400/20 bg-red-400/10 px-3 py-1.5 text-xs text-red-300 hover:bg-red-400/20"
        >
          <Trash2 size={11} />
          {t('resumes.select.deleteSelected')}
        </Button>
      </div>
    </motion.div>
  );
}
