import { useTranslation } from '@ajh/translations';
import { Button, Dropdown } from '@ajh/ui';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';

import { CliAgentInstall } from '../CliAgentInstall';

interface Props {
  provider: AiProvider;
  /** Whether the agent's CLI binary was detected. */
  connected: boolean;
  /** Models fetched via the provider (CLI agents return their aliases). */
  expandedModels: Array<{ name: string }>;
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
 * API key. Mirrors the cloud config's model dropdown (CLIs have no list endpoint,
 * so options are the agent's known aliases). Agents that support a reasoning effort
 * (Codex) also get an effort dropdown.
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
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const providerConfig = useAiProviderConfig();
  const efforts = meta.efforts ?? [];
  const currentEffort = providerConfig?.providers?.[provider]?.effort ?? '';

  const modelOptions =
    expandedModels.length > 0
      ? expandedModels.map((m) => ({ value: m.name, label: m.name }))
      : meta.models.map((m) => ({ value: m, label: m }));

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
              options={modelOptions}
              value={providerModel}
              onChange={onSelect}
              placeholder="Select a model…"
            />
          </div>

          {efforts.length > 0 && (
            <div className="space-y-1.5">
              <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
                {t('settings.aiProvider.reasoningEffort')}
              </div>
              <Dropdown
                options={[
                  { value: '', label: t('settings.aiProvider.effortDefault') },
                  ...efforts.map((e) => ({ value: e, label: e })),
                ]}
                value={currentEffort}
                onChange={(value) => setProviderSettings(provider, { effort: value })}
                placeholder={t('settings.aiProvider.effortDefault')}
              />
            </div>
          )}

          <Button
            variant="glass"
            size="sm"
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
