import type { ProviderModelInfo } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown } from '@ajh/ui';

import { EffortPicker } from '@/components/ui/EffortPicker';
import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

import { CliAgentInstall } from '../CliAgentInstall';

interface Props {
  provider: AiProvider;
  /** Whether the agent's CLI binary was detected. */
  connected: boolean;
  /** Models fetched via the provider — live discovery when the CLI offers it
   *  (e.g. Codex's `codex debug models`), else the curated `source: 'fallback'`
   *  aliases (see `CliAgentClient::list_models` in the Rust backend). */
  expandedModels: ProviderModelInfo[];
  providerModel: string;
  onSelect: (model: string) => void;
  onSetActive: () => void;
  isActive: boolean;
  /** Open the agent's install/setup docs. */
  onInstall: () => void;
  /** Re-probe detection (re-runs the system health check). */
  onRecheck: () => void;
}

/**
 * Config UI for a `cli-agent` provider: a locally-installed headless tool with no
 * API key. Model options come from the provider's live catalogue when the CLI can
 * enumerate one, else its curated aliases (labelled below when that's the case).
 * Agents that support a reasoning effort (Codex) also get an effort dropdown.
 */
export function CliAgentConfig({
  provider,
  connected,
  expandedModels,
  providerModel,
  onSelect,
  onSetActive,
  isActive,
  onInstall,
  onRecheck,
}: Props) {
  const { t } = useTranslation();
  const meta = PROVIDERS[provider];

  const modelOptions =
    expandedModels.length > 0
      ? expandedModels.map((m) => ({ value: m.name, label: m.displayName ?? m.name }))
      : meta.models.map((m) => ({ value: m, label: m }));
  // Every entry in one `expandedModels` response shares the same source (one
  // backend call) — the first is representative of the whole list.
  const usingFallbackList = expandedModels[0]?.source === 'fallback';

  // Keep a stored selection that fell out of the curated/live list selectable
  // — otherwise the trigger falls back to the placeholder and reads as a
  // reset config, when it's really just an unlisted model id (self-heals
  // once the live list includes it again).
  const options =
    providerModel && !modelOptions.some((o) => o.value === providerModel)
      ? [...modelOptions, { value: providerModel, label: providerModel }]
      : modelOptions;

  return (
    <>
      {!connected && (
        <CliAgentInstall
          provider={provider}
          label={meta.label}
          onGuide={onInstall}
          onRecheck={onRecheck}
        />
      )}

      {connected && (
        <>
          <div className="space-y-1.5">
            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
              {t('settings.aiModel.title')}
            </div>
            <Dropdown
              options={options}
              value={providerModel}
              onChange={onSelect}
              placeholder="Select a model…"
            />
            {usingFallbackList && (
              <p role="status" aria-live="polite" className="text-[10px] text-foreground/40">
                {t('models.cli.fallbackList')}
              </p>
            )}
          </div>

          <EffortPicker provider={provider} model={providerModel} />

          <Button
            variant="glass"
            onClick={onSetActive}
            disabled={isActive}
            className={isActive ? 'opacity-40' : 'ring-1 ring-brand/20'}
          >
            {isActive ? 'Currently active' : 'Set as active'}
          </Button>
        </>
      )}
    </>
  );
}
