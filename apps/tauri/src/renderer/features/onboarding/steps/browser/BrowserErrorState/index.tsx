import { ArrowLeft, X } from 'lucide-react';
import { motion } from 'motion/react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface BrowserErrorStateProps {
  onBack: () => void;
  onNext: () => void;
}

export function BrowserErrorState({ onBack, onNext }: BrowserErrorStateProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <div className="mb-6 flex justify-center">
        <motion.div
          initial={{ scale: 0.8, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          transition={{ duration: 0.4 }}
          className="relative"
        >
          <div className="absolute inset-0 rounded-full bg-orange-500/20 blur-xl" />
          <div className="relative flex h-16 w-16 items-center justify-center rounded-2xl bg-orange-500/10 ring-1 ring-orange-500/30">
            <X size={28} className="text-orange-400" />
          </div>
        </motion.div>
      </div>
      <div className="text-center">
        <h2 className="text-xl font-semibold text-foreground/95">
          {t('onboarding.browser.checkFailed')}
        </h2>
        <p className="mt-2 text-sm text-foreground/50">{t('onboarding.browser.checkFailedDesc')}</p>
      </div>
      <div className="flex items-center justify-between pt-4">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft size={14} /> {t('onboarding.back')}
        </Button>
        <Button onClick={onNext} className="px-6">
          {t('onboarding.browser.next')}
        </Button>
      </div>
    </div>
  );
}
