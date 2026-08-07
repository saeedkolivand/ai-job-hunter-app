import { Gauge } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Dropdown } from '@ajh/ui';

import { useModelCapabilities } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';

interface Props {
  provider: AiProvider;
  model: string;
  baseUrl?: string;
  /**
   * Inline variant for the model picker: no uppercase section heading, so the
   * control reads as a peer of the model dropdown rather than as a settings
   * section leaking into a panel. Settings surfaces leave this off.
   */
  compact?: boolean;
}

/**
 * Reasoning-effort dropdown, shared by every surface that selects a model — the
 * three provider-config sections (CLI agent, cloud HTTP, local Ollama) and
 * `ModelSelector`, which every other model-picking surface in the app renders.
 * Lives in `components/ui/` rather than under `features/settings/` precisely so
 * `ModelSelector` can reach it without a shared component depending on a feature.
 *
 * The offered levels come straight from the backend's `effortLevels` (via
 * `useModelCapabilities`) — a per-MODEL lookup, not a hardcoded per-provider
 * TS list: some providers' accepted level set genuinely varies by model tier
 * (Gemini's `thinkingLevel` — `gemini-3.1-flash-lite-image` only accepts
 * minimal/high, `gemini-3.1-pro-preview` also accepts medium), so a static list would be
 * wrong for some models. Hidden entirely while `effortLevels` is empty —
 * either the capability query hasn't resolved yet, or this model genuinely
 * doesn't support reasoning. That self-hiding is what makes it safe to render
 * unconditionally from `ModelSelector`: a non-reasoning model shows nothing.
 *
 * The stored value is per-PROVIDER (`providers[provider].effort`), not per-model
 * — so changing it here changes it for that provider everywhere, which is the
 * same preference the settings page writes.
 */
export function EffortPicker({ provider, model, baseUrl, compact = false }: Props) {
  const { t } = useTranslation();
  const caps = useModelCapabilities(provider, model, baseUrl);
  const effortLevels = caps.data?.effortLevels ?? [];
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const providerConfig = useAiProviderConfig();
  const currentEffort = providerConfig?.providers?.[provider]?.effort ?? '';

  if (effortLevels.length === 0) return null;

  return (
    <div className={compact ? 'mt-1.5' : 'space-y-1.5'}>
      {!compact && (
        <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.aiProvider.reasoningEffort')}
        </div>
      )}
      <Dropdown
        options={[
          { value: '', label: t('settings.aiProvider.effortDefault') },
          ...effortLevels.map((e) => ({ value: e, label: e })),
        ]}
        value={currentEffort}
        onChange={(value) => setProviderSettings(provider, { effort: value })}
        placeholder={t('settings.aiProvider.effortDefault')}
        // Compact mode drops the visible heading, so the control still needs an
        // accessible name — and a leading icon to read as "effort", not as a
        // second model dropdown. Mirrors ModelSelector's own `<Cpu>` idiom.
        {...(compact
          ? {
              icon: <Gauge size={13} />,
              'aria-label': t('settings.aiProvider.reasoningEffort'),
            }
          : {})}
      />
    </div>
  );
}
