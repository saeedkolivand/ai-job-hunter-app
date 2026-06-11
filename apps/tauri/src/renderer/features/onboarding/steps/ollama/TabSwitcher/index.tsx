import { Cloud, Computer, type LucideIcon, Terminal } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { Button, withDelay } from '@ajh/ui';

export type TabMode = 'local' | 'cloud' | 'cli';

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
    { id: 'cli', label: t('onboarding.ai.cliTab'), icon: Terminal },
  ];
  return (
    <motion.div
      initial={{ y: 10, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={withDelay(0.1)}
      className="mb-5 flex rounded-xl border border-white/[0.07] bg-white/[0.02] p-1"
    >
      {TABS.map(({ id, label, icon: Icon }) => (
        <Button
          key={id}
          variant="unstyled"
          onClick={() => onModeChange(id)}
          className={`flex flex-1 items-center justify-center gap-2 rounded-lg py-2 text-sm font-medium transition-all duration-150 ${
            mode === id
              ? 'bg-brand/15 text-brand-soft border border-brand/30'
              : 'text-foreground/40 hover:text-foreground/70'
          }`}
        >
          <Icon size={14} />
          {label}
        </Button>
      ))}
    </motion.div>
  );
}
