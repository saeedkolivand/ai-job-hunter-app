import { Cpu, Gauge, type LucideIcon, Zap } from 'lucide-react';
import { motion } from 'motion/react';

import { cn, GlassCard, SectionLabel, transition } from '@ajh/ui';

import type { PerformanceMode } from '@/store/preferences-schema';
import { usePerformanceMode, usePreferencesStore } from '@/store/preferences-store';

const PERFORMANCE_OPTIONS: {
  value: PerformanceMode;
  label: string;
  icon: LucideIcon;
  description: string;
  details: string[];
}[] = [
  {
    value: 'low-memory',
    label: 'Low Memory',
    icon: Cpu,
    description: 'Optimized for older hardware',
    details: ['Reduced blur intensity', 'Aggressive model unloading', 'Lower concurrency'],
  },
  {
    value: 'balanced',
    label: 'Balanced',
    icon: Gauge,
    description: 'Best performance for most systems',
    details: [
      'Default optimized behavior',
      'Smooth animations',
      'Efficient caching',
      'Balanced concurrency',
    ],
  },
  {
    value: 'performance',
    label: 'Performance',
    icon: Zap,
    description: 'Maximum speed on capable hardware',
    details: ['Higher concurrency', 'Richer visuals', 'Aggressive caching', 'Full animations'],
  },
];

export function PerformancePreferences() {
  const performanceMode = usePerformanceMode();
  const setPerformanceMode = usePreferencesStore((s) => s.setPerformanceMode);

  return (
    <GlassCard>
      <div className="mb-4">
        <SectionLabel>Performance Mode</SectionLabel>
      </div>
      <p className="mb-4 text-sm text-foreground/55">
        Optimize application performance based on your hardware capabilities.
      </p>
      <div className="grid gap-3">
        {PERFORMANCE_OPTIONS.map((opt) => {
          const Icon = opt.icon;
          const isSelected = performanceMode === opt.value;
          return (
            <motion.button
              key={opt.value}
              type="button"
              onClick={() => setPerformanceMode(opt.value)}
              className={cn(
                'relative flex items-start gap-4 rounded-xl border p-4 text-left transition-all duration-150',
                isSelected
                  ? 'border-brand-soft/50 bg-brand-soft/10 ring-1 ring-brand/20'
                  : 'border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10'
              )}
            >
              <div
                className={cn(
                  'rounded-xl p-3 transition-colors',
                  isSelected ? 'bg-brand-soft/20' : 'bg-white/5'
                )}
              >
                <Icon
                  size={24}
                  className={cn(
                    'transition-colors',
                    isSelected ? 'text-brand-soft' : 'text-foreground/40'
                  )}
                />
              </div>
              <div className="flex-1">
                <div
                  className={cn(
                    'text-base font-medium transition-colors',
                    isSelected ? 'text-foreground' : 'text-foreground/70'
                  )}
                >
                  {opt.label}
                </div>
                <div className="mb-2 text-sm text-foreground/40">{opt.description}</div>
                <ul className="space-y-1">
                  {opt.details.map((d) => (
                    <li key={d} className="text-xs text-foreground/30">
                      • {d}
                    </li>
                  ))}
                </ul>
              </div>
              {isSelected && (
                <motion.div
                  layoutId="performance-selection"
                  className="absolute inset-0 rounded-xl border-2 border-brand-soft/30"
                  transition={transition.selection}
                />
              )}
            </motion.button>
          );
        })}
      </div>
    </GlassCard>
  );
}
