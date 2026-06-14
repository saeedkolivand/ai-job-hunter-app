import { Cpu, Gauge, type LucideIcon, SlidersHorizontal, Zap } from 'lucide-react';
import { motion } from 'motion/react';

import { cn, Dropdown, GlassCard, SectionLabel, Switch, transition } from '@ajh/ui';

import {
  type BlurTier,
  PERFORMANCE_PRESETS,
  type PerformanceMode,
  type PerformanceProfile,
  type PerfTier,
} from '@/store/preferences-schema';
import {
  usePerformanceMode,
  usePreferencesStore,
  useResolvedPerformanceProfile,
} from '@/store/preferences-store';

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
  {
    value: 'custom',
    label: 'Custom',
    icon: SlidersHorizontal,
    description: 'Fine-tune every element',
    details: ['Per-effect visual toggles', 'Manual backend tiers', 'Tailored to your machine'],
  },
];

const BLUR_OPTIONS: { value: BlurTier; label: string }[] = [
  { value: 'full', label: 'Full' },
  { value: 'reduced', label: 'Reduced' },
  { value: 'off', label: 'Off' },
];

const CONCURRENCY_OPTIONS: { value: PerfTier; label: string }[] = [
  { value: 'low', label: 'Low' },
  { value: 'balanced', label: 'Balanced' },
  { value: 'high', label: 'High' },
];

const KEEP_ALIVE_OPTIONS: { value: PerfTier; label: string }[] = [
  { value: 'low', label: 'Unload immediately' },
  { value: 'balanced', label: 'Balanced' },
  { value: 'high', label: 'Keep warm' },
];

const CACHE_OPTIONS: { value: PerfTier; label: string }[] = [
  { value: 'low', label: 'Minimal' },
  { value: 'balanced', label: 'Balanced' },
  { value: 'high', label: 'Generous' },
];

export function PerformancePreferences() {
  const performanceMode = usePerformanceMode();
  const profile = useResolvedPerformanceProfile();
  const setPerformanceMode = usePreferencesStore((s) => s.setPerformanceMode);
  const setCustomPerformance = usePreferencesStore((s) => s.setCustomPerformance);
  const customPerformance = usePreferencesStore((s) => s.customPerformance);

  const selectMode = (value: PerformanceMode) => {
    setPerformanceMode(value);
    // Seed the custom profile from the balanced preset the first time the user
    // picks Custom, so the sub-panel controls have a concrete profile to edit.
    if (value === 'custom' && !customPerformance) {
      setCustomPerformance(structuredClone(PERFORMANCE_PRESETS.balanced));
    }
  };

  // Apply a single visual change on top of the current (custom) profile.
  const patchVisual = (patch: Partial<PerformanceProfile['visual']>) => {
    setCustomPerformance({ ...profile, visual: { ...profile.visual, ...patch } });
  };
  const patchBackend = (patch: Partial<PerformanceProfile['backend']>) => {
    setCustomPerformance({ ...profile, backend: { ...profile.backend, ...patch } });
  };

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
              onClick={() => selectMode(opt.value)}
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

      {performanceMode === 'custom' && (
        <div className="mt-5 space-y-5 rounded-xl border border-white/10 bg-white/5 p-4">
          {/* Visual */}
          <div className="space-y-3">
            <SectionLabel>Visual</SectionLabel>
            <Switch
              label="Aurora ribbons"
              description="Slow, wide hue-rotating background blobs"
              checked={profile.visual.aurora}
              onCheckedChange={(next) => patchVisual({ aurora: next })}
            />
            <Switch
              label="Nebulae"
              description="Medium accent blobs layered over the aurora"
              checked={profile.visual.nebula}
              onCheckedChange={(next) => patchVisual({ nebula: next })}
            />
            <Switch
              label="Cursor glow"
              description="A soft glow that trails the pointer"
              checked={profile.visual.cursorGlow}
              onCheckedChange={(next) => patchVisual({ cursorGlow: next })}
            />
            <Switch
              label="Rich animations"
              description="Animate the aurora and nebula layers"
              checked={profile.visual.animations}
              onCheckedChange={(next) => patchVisual({ animations: next })}
            />
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <label htmlFor="perf-blur" className="text-xs font-medium text-foreground/80">
                  Backdrop blur
                </label>
                <div className="text-[11px] text-foreground/45">
                  Frosted-glass intensity across surfaces
                </div>
              </div>
              <div className="w-44 shrink-0">
                <Dropdown
                  id="perf-blur"
                  options={BLUR_OPTIONS}
                  value={profile.visual.blur}
                  onChange={(v) => patchVisual({ blur: v as BlurTier })}
                />
              </div>
            </div>
          </div>

          {/* Backend */}
          <div className="space-y-3">
            <SectionLabel>Backend</SectionLabel>
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <label
                  htmlFor="perf-concurrency"
                  className="text-xs font-medium text-foreground/80"
                >
                  Concurrency
                </label>
                <div className="text-[11px] text-foreground/45">Parallel background workers</div>
              </div>
              <div className="w-44 shrink-0">
                <Dropdown
                  id="perf-concurrency"
                  options={CONCURRENCY_OPTIONS}
                  value={profile.backend.concurrency}
                  onChange={(v) => patchBackend({ concurrency: v as PerfTier })}
                />
              </div>
            </div>
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <label htmlFor="perf-keep-alive" className="text-xs font-medium text-foreground/80">
                  Model keep-alive
                </label>
                <div className="text-[11px] text-foreground/45">
                  How long idle models stay loaded
                </div>
              </div>
              <div className="w-44 shrink-0">
                <Dropdown
                  id="perf-keep-alive"
                  options={KEEP_ALIVE_OPTIONS}
                  value={profile.backend.keepAlive}
                  onChange={(v) => patchBackend({ keepAlive: v as PerfTier })}
                />
              </div>
            </div>
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <label htmlFor="perf-cache" className="text-xs font-medium text-foreground/80">
                  Cache
                </label>
                <div className="text-[11px] text-foreground/45">Cached result retention</div>
              </div>
              <div className="w-44 shrink-0">
                <Dropdown
                  id="perf-cache"
                  options={CACHE_OPTIONS}
                  value={profile.backend.cache}
                  onChange={(v) => patchBackend({ cache: v as PerfTier })}
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </GlassCard>
  );
}
