import { Eye, EyeOff, Inbox, Key, Loader2, PenLine, Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import type { ProviderModelInfo } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, EmptyState, ErrorState, Input, useNotification } from '@ajh/ui';

import { sortModelsNewestFirst } from '@/lib/ai-providers/model-sort';
import { useSetProviderSettings } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

import { EffortPicker } from '../EffortPicker';

interface ProviderMeta {
  label: string;
  description: string;
  docsUrl: string;
  color: string;
}

interface Props {
  provider: AiProvider;
  meta: ProviderMeta;
  connected: boolean;
  isSaving: boolean;
  isTesting?: boolean;
  providerModel: string;
  expandedModels: ProviderModelInfo[];
  /** Still fetching the expanded row's model list. */
  expandedModelsLoading?: boolean;
  /** `expandedModels` came from the last-good local cache (live fetch failed). */
  expandedModelsCached?: boolean;
  /** Live fetch failed AND no cache was available — the real failure message. */
  expandedModelsError?: string;
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
  onRecheck?: () => void;
}

export function CloudProviderConfig({
  provider,
  meta,
  connected,
  isSaving,
  providerModel,
  expandedModels,
  expandedModelsLoading = false,
  expandedModelsCached = false,
  expandedModelsError,
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
  onRecheck,
}: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setProviderSettings = useSetProviderSettings();
  const [changing, setChanging] = useState(false);

  const modelOptions = sortModelsNewestFirst(expandedModels).map((m) => ({
    value: m.name,
    label: m.displayName ?? m.name,
  }));

  // Keep a stored selection that fell out of the live/cached list selectable
  // — otherwise the trigger falls back to the placeholder and reads as a
  // reset config, when it's really just an unlisted model id (self-heals
  // once the live list includes it again).
  const options =
    providerModel && !modelOptions.some((o) => o.value === providerModel)
      ? [...modelOptions, { value: providerModel, label: providerModel }]
      : modelOptions;

  // Collapse the editor only after a save cycle COMPLETES (isSaving true→false)
  // and the parent has cleared the input (its success signal). Gating on the
  // falling edge is what stops the editor snapping shut the instant the user
  // clicks "Change key", where apiKeyInput is still empty and no save has run.
  const wasSaving = useRef(false);
  useEffect(() => {
    if (wasSaving.current && !isSaving && apiKeyInput === '') setChanging(false);
    wasSaving.current = isSaving;
  }, [isSaving, apiKeyInput]);

  return (
    <>
      {connected ? (
        changing ? (
          <div className="space-y-2">
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
                aria-label={t(
                  showKey ? 'settings.aiProvider.hideKey' : 'settings.aiProvider.showKey'
                )}
                onClick={onToggleShowKey}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </Button>
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="ghost"
                className="text-xs"
                onClick={() => {
                  setChanging(false);
                  onApiKeyChange('');
                }}
              >
                {t('settings.cancel')}
              </Button>
              <Button
                variant="glass"
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
        ) : (
          <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
            <div className="flex items-center gap-2 text-sm text-emerald-300/80">
              <Key size={12} /> {t('settings.aiProvider.keyStored')}
            </div>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                className="text-xs text-foreground/50 hover:text-foreground/80"
                onClick={() => setChanging(true)}
              >
                <PenLine size={11} /> {t('settings.aiProvider.changeKey')}
              </Button>
              <Button
                variant="ghost"
                className="text-xs text-red-400/60 hover:text-red-400"
                onClick={() => void onRemoveKey()}
              >
                <Trash2 size={11} /> {t('settings.aiProvider.removeKey')}
              </Button>
            </div>
          </div>
        )
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
                aria-label={t(
                  showKey ? 'settings.aiProvider.hideKey' : 'settings.aiProvider.showKey'
                )}
                onClick={onToggleShowKey}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
              >
                {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </Button>
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="glass"
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
              className="shrink-0"
              onClick={() =>
                setProviderSettings.mutate(
                  {
                    provider: 'openai-compatible',
                    baseUrl: baseUrlInput || undefined,
                  },
                  {
                    onError: () =>
                      notify.error({ message: t('settings.aiProvider.saveUrlFailed') }),
                  }
                )
              }
            >
              {t('settings.aiProvider.saveUrl')}
            </Button>
          </div>
        </div>
      )}

      {/* Model selector — four distinct states beyond the normal dropdown:
          (1) still loading with nothing to show yet → a labelled spinner (not
          a bare empty dropdown, which reads as "zero models"), (2) live fetch
          failed with no cache and no stored selection to fall back to → the
          real failure, (3) fetch succeeded but the catalogue is genuinely
          empty → a neutral empty state, (4) options.length > 0 (fresh,
          cached, or a preserved unlisted selection) → the dropdown, with a
          small note when the list is cached. Loading is checked FIRST — it's
          the only state where none of the other three guards fire, so it used
          to fall through to an unlabelled empty `<Dropdown>`. */}
      {connected && (
        <div className="space-y-1.5">
          <div className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {t('settings.aiModel.title')}
          </div>
          {options.length === 0 && expandedModelsLoading ? (
            <div
              role="status"
              aria-live="polite"
              className="flex items-center gap-2 text-xs text-foreground/40"
            >
              <Loader2 size={13} className="animate-spin" />
              {t('settings.aiModel.loading')}
            </div>
          ) : options.length === 0 && expandedModelsError ? (
            <ErrorState
              title={t('settings.aiModel.fetchFailedTitle')}
              description={expandedModelsError}
              onRetry={onRecheck}
              className="py-6"
            />
          ) : options.length === 0 ? (
            <EmptyState
              icon={Inbox}
              title={t('settings.aiModel.emptyTitle')}
              description={t('settings.aiModel.emptyDescription')}
              className="py-6"
            />
          ) : (
            <>
              <Dropdown
                options={options}
                value={providerModel}
                onChange={onSelectModel}
                placeholder="Select a model…"
              />
              {expandedModelsCached && (
                <p role="status" aria-live="polite" className="text-[10px] text-foreground/40">
                  {t('settings.aiModel.cachedNotice')}
                </p>
              )}
            </>
          )}
        </div>
      )}

      {/* `openai-compatible` never reports effort levels — the backend
          deliberately never guesses reasoning support for an unknown gateway
          catalog (`OpenAiClient::supports_reasoning_effort` is a hard
          `false` for it) — so the picker can never render for it anyway.
          Skipping it here (instead of passing a `baseUrl` that would make
          `useModelCapabilities`'s query key change on every un-debounced
          keystroke in the base-URL input) avoids firing a capability probe
          per keystroke for a picker that would stay empty regardless. */}
      {connected && provider !== 'openai-compatible' && (
        <EffortPicker provider={provider} model={providerModel} />
      )}

      {/* Set active button */}
      {connected && (
        <Button
          variant="glass"
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
