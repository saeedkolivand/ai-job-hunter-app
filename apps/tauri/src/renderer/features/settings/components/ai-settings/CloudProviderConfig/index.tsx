import { Eye, EyeOff, Key, Loader2, Trash2 } from 'lucide-react';

import { Button, Dropdown, Input } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import type { AiProvider } from '@/store/preferences-schema';
import { usePreferencesStore } from '@/store/preferences-store';

interface ProviderMeta {
  label: string;
  description: string;
  docsUrl: string;
  color: string;
  models: string[];
}

interface Props {
  provider: AiProvider;
  meta: ProviderMeta;
  connected: boolean;
  isSaving: boolean;
  isTesting?: boolean;
  providerModel: string;
  expandedModels: Array<{ name: string }>;
  apiKeyInput: string;
  showKey: boolean;
  baseUrlInput: string;
  onApiKeyChange: (value: string) => void;
  onToggleShowKey: () => void;
  onBaseUrlChange: (value: string) => void;
  onSaveKey: () => void;
  onRemoveKey: () => void;
  onTestKey?: () => void;
  onSelectModel: (model: string) => void;
  onSetActive: () => void;
  isActive: boolean;
  onOpenDocs: () => void;
}

export function CloudProviderConfig({
  provider,
  meta,
  connected,
  isSaving,
  providerModel,
  expandedModels,
  apiKeyInput,
  showKey,
  baseUrlInput,
  onApiKeyChange,
  onToggleShowKey,
  onBaseUrlChange,
  onSaveKey,
  onRemoveKey,
  onSelectModel,
  onSetActive,
  isActive,
  onOpenDocs,
}: Props) {
  const { t } = useTranslation();
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);

  return (
    <>
      {connected ? (
        <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
          <div className="flex items-center gap-2 text-sm text-emerald-300/80">
            <Key size={12} /> {t('settings.aiProvider.keyStored')}
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="text-xs text-red-400/60 hover:text-red-400"
            onClick={() => void onRemoveKey()}
          >
            <Trash2 size={11} /> {t('settings.aiProvider.removeKey')}
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          <p className="text-xs text-foreground/40">
            {t('settings.aiProvider.getKeyAt')}{' '}
            <Button
              variant="unstyled"
              onClick={onOpenDocs}
              className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
            >
              {meta.docsUrl.replace('https://', '')}
            </Button>
          </p>
          <div className="flex flex-col gap-2">
            <div className="relative">
              <Input
                type={showKey ? 'text' : 'password'}
                value={apiKeyInput}
                onChange={(e) => onApiKeyChange(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void onSaveKey()}
                placeholder={t('settings.aiProvider.keyPlaceholder')}
                className="w-full pr-9 text-sm"
              />
              <Button
                variant="unstyled"
                onClick={onToggleShowKey}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </Button>
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="glass"
                size="sm"
                disabled={!apiKeyInput.trim() || isSaving}
                onClick={() => void onSaveKey()}
                className={apiKeyInput.trim() && !isSaving ? 'ring-1 ring-brand/20' : ''}
              >
                {isSaving ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  t('settings.aiProvider.saveKey')
                )}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Base URL for openai-compatible */}
      {provider === 'openai-compatible' && (
        <div className="space-y-1.5">
          <label className="text-xs font-semibold uppercase tracking-widest text-foreground/55">
            {t('settings.aiProvider.baseUrl')}
          </label>
          <div className="flex gap-2">
            <Input
              value={baseUrlInput}
              onChange={(e) => onBaseUrlChange(e.target.value)}
              placeholder="https://api.groq.com/openai/v1"
              className="flex-1 text-sm"
            />
            <Button
              variant="ghost"
              size="sm"
              className="shrink-0"
              onClick={() =>
                setProviderSettings('openai-compatible', {
                  baseUrl: baseUrlInput || undefined,
                })
              }
            >
              {t('settings.aiProvider.saveUrl')}
            </Button>
          </div>
        </div>
      )}

      {/* Model selector */}
      {connected && (
        <div className="space-y-1.5">
          <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.aiModel.title')}
          </div>
          <Dropdown
            options={
              expandedModels.length > 0
                ? expandedModels.map((m) => ({ value: m.name, label: m.name }))
                : (meta.models ?? []).map((n) => ({
                    value: n,
                    label: n,
                  }))
            }
            value={providerModel}
            onChange={onSelectModel}
            placeholder="Select a model…"
          />
        </div>
      )}

      {/* Set active button */}
      {connected && (
        <Button
          variant="glass"
          size="sm"
          onClick={onSetActive}
          disabled={isActive}
          className={isActive ? 'opacity-40' : 'glow-subtle'}
        >
          {isActive ? 'Currently active' : 'Set as active'}
        </Button>
      )}
    </>
  );
}
