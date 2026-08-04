import { useTranslation } from '@ajh/translations';
import { Dropdown } from '@ajh/ui';

import { useModelCapabilities } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';

interface Props {
  provider: AiProvider;
  model: string;
  baseUrl?: string;
}

/**
 * Reasoning-effort dropdown, shared by every provider-config surface (CLI
 * agent, cloud HTTP, local Ollama) so the picker logic lives once instead of
 * drifting across three copies.
 *
 * The offered levels come straight from the backend's `effortLevels` (via
 * `useModelCapabilities`) — a per-MODEL lookup, not a hardcoded per-provider
 * TS list: some providers' accepted level set genuinely varies by model tier
 * (Gemini's `thinkingLevel` — `gemini-3-pro-preview` only accepts low/high,
 * `gemini-3.1-pro-preview` also accepts medium), so a static list would be
 * wrong for some models. Hidden entirely while `effortLevels` is empty —
 * either the capability query hasn't resolved yet, or this model genuinely
 * doesn't support reasoning.
 */
export function EffortPicker({ provider, model, baseUrl }: Props) {
  const { t } = useTranslation();
  const caps = useModelCapabilities(provider, model, baseUrl);
  const effortLevels = caps.data?.effortLevels ?? [];
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const providerConfig = useAiProviderConfig();
  const currentEffort = providerConfig?.providers?.[provider]?.effort ?? '';

  if (effortLevels.length === 0) return null;

  return (
    <div className="space-y-1.5">
      <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
        {t('settings.aiProvider.reasoningEffort')}
      </div>
      <Dropdown
        options={[
          { value: '', label: t('settings.aiProvider.effortDefault') },
          ...effortLevels.map((e) => ({ value: e, label: e })),
        ]}
        value={currentEffort}
        onChange={(value) => setProviderSettings(provider, { effort: value })}
        placeholder={t('settings.aiProvider.effortDefault')}
      />
    </div>
  );
}
