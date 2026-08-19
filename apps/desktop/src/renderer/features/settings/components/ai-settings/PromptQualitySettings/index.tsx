import { AlertTriangle, Gauge, SlidersHorizontal, Zap } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { SegmentedControl, SettingsSection } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider, PromptQuality } from '@/store/preferences-schema';
import { usePreferencesStore, usePromptQuality } from '@/store/preferences-store';

interface Props {
  /** Currently active provider. `resolveEffectiveTier`
   *  (`lib/generate/provider-context.ts`) returns `'large'` unconditionally for
   *  anything but `'ollama'`, ignoring the preference below entirely — passed
   *  down so the copy can stay honest about when the choice actually changes
   *  generation, instead of shipping an unqualified toggle. */
  activeProvider: AiProvider;
}

const OPTIONS: { value: PromptQuality; labelKey: string; icon?: typeof Gauge }[] = [
  { value: 'full', labelKey: 'aiGenerate.wizard.quality.full', icon: SlidersHorizontal },
  { value: 'auto', labelKey: 'aiGenerate.wizard.quality.auto' },
  { value: 'compact', labelKey: 'aiGenerate.wizard.quality.fast', icon: Zap },
];

/**
 * Canonical home for the global `promptQuality` preference. Reads/writes the
 * SAME `usePreferencesStore` field as the two other surfaces that can change
 * it (`StepFineTune`, `AnalyzeLeftPanel`) — all three stay in sync
 * automatically since there is only one store field.
 */
export function PromptQualitySettings({ activeProvider }: Props) {
  const { t } = useTranslation();
  const promptQuality = usePromptQuality();
  const setPromptQuality = usePreferencesStore((s) => s.setPromptQuality);
  const isOllama = activeProvider === 'ollama';

  return (
    <SettingsSection icon={Gauge} label={t('settings.promptQuality.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.promptQuality.description')}
      </p>
      <SegmentedControl<PromptQuality>
        variant="grid"
        ariaLabel={t('settings.promptQuality.title')}
        value={promptQuality}
        onChange={setPromptQuality}
        options={OPTIONS.map((o) => ({ value: o.value, label: t(o.labelKey), icon: o.icon }))}
      />
      {!isOllama && (
        <div className="mt-3 flex items-start gap-2 rounded-lg border border-[var(--border-clear)] bg-foreground/[0.03] px-3 py-2">
          <AlertTriangle size={12} className="mt-0.5 shrink-0 text-foreground/40" />
          <p className="text-[10px] leading-relaxed text-foreground/45">
            {t('settings.promptQuality.ollamaOnlyNote', {
              provider: PROVIDERS[activeProvider].label,
            })}
          </p>
        </div>
      )}
    </SettingsSection>
  );
}
