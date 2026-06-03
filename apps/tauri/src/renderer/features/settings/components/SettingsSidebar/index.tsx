import { ChevronRight } from 'lucide-react';
import { motion } from 'motion/react';

import { cn, transition } from '@ajh/ui';

import type { NavGroup, SectionId } from '@/features/settings/constants';

interface Props {
  navGroups: NavGroup[];
  activeSection: SectionId;
  onSectionChange: (id: SectionId) => void;
}

export function SettingsSidebar({ navGroups, activeSection, onSectionChange }: Props) {
  return (
    <aside className="flex w-56 shrink-0 flex-col gap-6 overflow-y-auto border-white/[0.05] px-3 py-8">
      {navGroups.map((group) => (
        <div key={group.label}>
          <div className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-widest text-foreground/55">
            {group.label}
          </div>
          <nav className="flex flex-col gap-1">
            {group.items.map(({ id, label, icon: Icon }) => {
              const active = activeSection === id;
              return (
                <div key={id} className="relative">
                  {active && (
                    <motion.div
                      layoutId="settings-pill"
                      className="absolute inset-0 rounded-xl bg-white/[0.07]"
                      transition={transition.spring}
                    />
                  )}
                  <div
                    role="button"
                    onClick={() => onSectionChange(id)}
                    className={cn(
                      'group relative flex cursor-pointer items-center gap-2.5 rounded-xl px-3 py-2 text-sm transition-colors duration-150',
                      active
                        ? 'text-foreground'
                        : 'text-foreground/45 hover:bg-white/[0.04] hover:text-foreground/75'
                    )}
                  >
                    <Icon
                      size={15}
                      className={cn(
                        'shrink-0 transition-colors duration-150',
                        active
                          ? 'text-foreground/70'
                          : 'text-foreground/35 group-hover:text-foreground/55'
                      )}
                    />
                    <span className="flex-1 font-medium">{label}</span>
                    {active && <ChevronRight size={12} className="text-foreground/30" />}
                  </div>
                </div>
              );
            })}
          </nav>
        </div>
      ))}
    </aside>
  );
}
