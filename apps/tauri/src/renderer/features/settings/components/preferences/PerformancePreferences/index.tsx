import { Cpu, Gauge, Info, type LucideIcon, SlidersHorizontal, Zap } from 'lucide-react';
import { motion } from 'motion/react';

import { type TFunction, useTranslation } from '@ajh/translations';
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

/** Mode cards, in display order. Labels/copy resolved from i18n at render. */
const PERFORMANCE_MODE_META: { value: PerformanceMode; icon: LucideIcon; i18nKey: string }[] = [
  { value: 'low-memory', icon: Cpu, i18nKey: 'lowMemory' },
  { value: 'balanced', icon: Gauge, i18nKey: 'balanced' },
  { value: 'performance', icon: Zap, i18nKey: 'performance' },
  { value: 'custom', icon: SlidersHorizontal, i18nKey: 'custom' },
];

const BLUR_TIERS: BlurTier[] = ['full', 'reduced', 'off'];
/** The three generic backend dropdowns share the same tier order. */
const BACKEND_TIERS: PerfTier[] = ['low', 'balanced', 'high'];
/** The three backend knobs, in display order. */
const BACKEND_KNOBS = ['concurrency', 'keepAlive', 'cache'] as const;
type BackendKnob = (typeof BACKEND_KNOBS)[number];

/**
 * Maps each backend knob to the resolved profile field it edits. Keeps the
 * generic render loop (below) decoupled from the concrete profile shape.
 */
const KNOB_FIELD: Record<BackendKnob, keyof PerformanceProfile['backend']> = {
  concurrency: 'concurrency',
  keepAlive: 'keepAlive',
  cache: 'cache',
};

/**
 * Per-tier "what this actually does" copy for one backend knob. Surfaces the
 * concrete values the renderer pushes over IPC (see `resolveBackendConfig` in
 * preferences-schema.ts): concurrency 1/2/4 workers, keep-alive 0/300/1800s,
 * cache 250/2000/unlimited rows at 1d/7d/no-expiry. Shown as helper text for the
 * selected tier plus a hover popover listing all three.
 */
function BackendOptionInfo({ t, knob }: { t: TFunction; knob: BackendKnob }) {
  const base = `settings.performanceMode.backend.${knob}`;
  return (
    <div className="relative group shrink-0">
      <Info
        size={12}
        className="text-foreground/30 cursor-help transition-colors group-hover:text-foreground/60"
      />
      <div className="absolute right-0 top-full z-50 mt-2 w-64 rounded-xl border border-foreground/15 bg-[var(--color-card)] p-3 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 shadow-2xl">
        <p className="mb-2 text-[11px] leading-relaxed text-foreground/70">{t(`${base}.info`)}</p>
        <ul className="space-y-1">
          {BACKEND_TIERS.map((tier) => (
            <li key={tier} className="text-[11px] leading-snug text-foreground/50">
              {t(`${base}.details.${tier}`)}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

export function PerformancePreferences() {
  const { t } = useTranslation();
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

  const blurOptions = BLUR_TIERS.map((tier) => ({
    value: tier,
    label: t(`settings.performanceMode.visual.blur.options.${tier}`),
  }));

  return (
    <GlassCard>
      <div className="mb-4">
        <SectionLabel>{t('settings.performanceMode.heading')}</SectionLabel>
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.performanceMode.subheading')}</p>
      <div className="grid gap-3">
        {PERFORMANCE_MODE_META.map((opt) => {
          const Icon = opt.icon;
          const isSelected = performanceMode === opt.value;
          const base = `settings.performanceMode.options.${opt.i18nKey}`;
          const details = t(`${base}.details`, { returnObjects: true }) as string[];
          return (
            <motion.button
              key={opt.value}
              type="button"
              onClick={() => selectMode(opt.value)}
              className={cn(
                'relative flex items-start gap-4 rounded-xl border p-4 text-left transition-all duration-150',
                isSelected
                  ? 'border-brand-soft/50 bg-brand-soft/10 ring-1 ring-brand/20'
                  : 'border-foreground/10 bg-foreground/[0.03] hover:border-foreground/20 hover:bg-foreground/[0.06]'
              )}
            >
              <div
                className={cn(
                  'rounded-xl p-3 transition-colors',
                  isSelected ? 'bg-brand-soft/20' : 'bg-foreground/[0.06]'
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
                  {t(`${base}.label`)}
                </div>
                <div className="mb-2 text-sm text-foreground/40">{t(`${base}.description`)}</div>
                <ul className="space-y-1">
                  {details.map((d) => (
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
        <div className="mt-5 space-y-5 rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4">
          {/* Visual */}
          <div className="space-y-3">
            <SectionLabel>{t('settings.performanceMode.visual.heading')}</SectionLabel>
            <Switch
              label={t('settings.performanceMode.visual.aurora.label')}
              description={t('settings.performanceMode.visual.aurora.description')}
              checked={profile.visual.aurora}
              onCheckedChange={(next) => patchVisual({ aurora: next })}
            />
            <Switch
              label={t('settings.performanceMode.visual.nebula.label')}
              description={t('settings.performanceMode.visual.nebula.description')}
              checked={profile.visual.nebula}
              onCheckedChange={(next) => patchVisual({ nebula: next })}
            />
            <Switch
              label={t('settings.performanceMode.visual.cursorGlow.label')}
              description={t('settings.performanceMode.visual.cursorGlow.description')}
              checked={profile.visual.cursorGlow}
              onCheckedChange={(next) => patchVisual({ cursorGlow: next })}
            />
            <Switch
              label={t('settings.performanceMode.visual.animations.label')}
              description={t('settings.performanceMode.visual.animations.description')}
              checked={profile.visual.animations}
              onCheckedChange={(next) => patchVisual({ animations: next })}
            />
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <label htmlFor="perf-blur" className="text-xs font-medium text-foreground/80">
                  {t('settings.performanceMode.visual.blur.label')}
                </label>
                <div className="text-[11px] text-foreground/45">
                  {t('settings.performanceMode.visual.blur.description')}
                </div>
              </div>
              <div className="w-44 shrink-0">
                <Dropdown
                  id="perf-blur"
                  options={blurOptions}
                  value={profile.visual.blur}
                  onChange={(v) => patchVisual({ blur: v as BlurTier })}
                />
              </div>
            </div>
          </div>

          {/* Backend */}
          <div className="space-y-3">
            <SectionLabel>{t('settings.performanceMode.backend.heading')}</SectionLabel>
            {BACKEND_KNOBS.map((knob) => {
              const base = `settings.performanceMode.backend.${knob}`;
              const field = KNOB_FIELD[knob];
              const selectedTier = profile.backend[field];
              const id = `perf-${knob === 'keepAlive' ? 'keep-alive' : knob}`;
              const options = BACKEND_TIERS.map((tier) => ({
                value: tier,
                label: t(`${base}.options.${tier}`),
              }));
              return (
                <div key={knob} className="space-y-1.5">
                  <div className="flex items-center justify-between gap-4">
                    <div className="min-w-0">
                      <div className="flex items-center gap-1.5">
                        <label htmlFor={id} className="text-xs font-medium text-foreground/80">
                          {t(`${base}.label`)}
                        </label>
                        <BackendOptionInfo t={t} knob={knob} />
                      </div>
                      <div className="text-[11px] text-foreground/45">
                        {t(`${base}.description`)}
                      </div>
                    </div>
                    <div className="w-44 shrink-0">
                      <Dropdown
                        id={id}
                        options={options}
                        value={selectedTier}
                        onChange={(v) => patchBackend({ [field]: v as PerfTier })}
                      />
                    </div>
                  </div>
                  {/* Concrete effect of the currently-selected tier. */}
                  <div className="text-[11px] text-foreground/35">
                    {t(`${base}.details.${selectedTier}`)}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </GlassCard>
  );
}
