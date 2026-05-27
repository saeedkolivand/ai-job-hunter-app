import { Cloud, Computer, type LucideIcon } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@/lib/i18n';

type TabMode = 'local' | 'cloud';

interface TabOption {
  id: TabMode;
  label: string;
  icon: LucideIcon;
}

interface TabSwitcherProps {
  mode: TabMode;
  onModeChange: (mode: TabMode) => void;
}

export function TabSwitcher({ mode, onModeChange }: TabSwitcherProps) {
  const { t } = useTranslation();

  const TABS: TabOption[] = [
    { id: 'local', label: t('onboarding.ai.localTab'), icon: Computer },
    { id: 'cloud', label: t('onboarding.ai.cloudTab'), icon: Cloud },
  ];
  return (
    <motion.div
      initial={{ y: 10, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ delay: 0.1 }}
      className="mb-5 flex rounded-xl border border-white/[0.07] bg-white/[0.02] p-1"
    >
      {TABS.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          onClick={() => onModeChange(id)}
          className={`flex flex-1 items-center justify-center gap-2 rounded-lg py-2 text-sm font-medium transition-all duration-150 ${
            mode === id
              ? 'bg-brand/15 text-brand-soft border border-brand/30'
              : 'text-foreground/40 hover:text-foreground/70'
          }`}
        >
          <Icon size={14} />
          {label}
        </button>
      ))}
    </motion.div>
  );
}
