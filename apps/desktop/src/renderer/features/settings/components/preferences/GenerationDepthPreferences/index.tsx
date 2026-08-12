import { useTranslation } from '@ajh/translations';
import { GlassCard, SectionLabel } from '@ajh/ui';

import { DepthSelector, useSmallModelWarning } from '@/components/generation/DepthSelector';
import { useGenerationDepth, usePreferencesStore } from '@/store/preferences-store';

/**
 * The DEFAULT generation depth. Every generate/tailor surface shows the same
 * picker and can override it for one run, so this card sets the starting point
 * rather than a lock — which is why it reuses the shared `DepthSelector`
 * (same labels, same self-describing tooltips, same info popover) instead of a
 * settings-shaped copy that could drift from it.
 */
export function GenerationDepthPreferences() {
  const { t } = useTranslation();
  const depth = useGenerationDepth();
  const setGenerationDepth = usePreferencesStore((s) => s.setGenerationDepth);
  const smallModel = useSmallModelWarning();

  return (
    <GlassCard>
      <div className="mb-4">
        <SectionLabel>{t('settings.generationDepth.title')}</SectionLabel>
      </div>
      <p className="mb-4 text-sm text-foreground/55">{t('settings.generationDepth.description')}</p>
      <DepthSelector
        value={depth}
        onChange={setGenerationDepth}
        smallModel={smallModel}
        hideLabel
      />
    </GlassCard>
  );
}
