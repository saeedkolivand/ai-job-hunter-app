import { Bot, CheckCircle2, Key, Loader2, RefreshCw, WifiOff } from 'lucide-react';

import type { ProviderModelInfo } from '@ajh/shared';
import { Button } from '@ajh/ui';

import type { ProviderMeta } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { CliAgentConfig } from '../CliAgentConfig';
import { CloudProviderConfig } from '../CloudProviderConfig';
import { OllamaConfig } from '../OllamaConfig';

interface Props {
  provider: AiProvider;
  meta: ProviderMeta;
  connected: boolean;
  isActive: boolean;
  isExpanded: boolean;
  isSaving: boolean;
  isTesting?: boolean;
  providerModel: string;
  ollamaModels: Model[];
  expandedModels: ProviderModelInfo[];
  /** Cloud/CLI model list still loading for the expanded row. */
  expandedModelsLoading: boolean;
  /** `expandedModels` was served from the local last-good cache (live fetch failed). */
  expandedModelsCached: boolean;
  /** Live fetch failed AND no cache was available — the real failure message. */
  expandedModelsError?: string;
  loadingOllama: boolean;
  pulling: string | null;
  apiKeyInput: string;
  showKey: boolean;
  baseUrlInput: string;
  /** The resolved openai-compatible base URL (in-progress edit, else saved) —
   *  computed once in `useProviderKeys`, so the "is this provider configured"
   *  check downstream never disagrees with the model-fetch it gates. */
  configuredBaseUrl?: string;
  onToggleExpand: () => void;
  onTestKey?: () => void;
  onRemoveKey: () => void;
  onSelectModel: (provider: AiProvider, model: string) => void;
  onPullOllama: (model: string) => void;
  onSetActive: () => void;
  onApiKeyChange: (value: string) => void;
  onToggleShowKey: () => void;
  onBaseUrlChange: (value: string) => void;
  onSaveKey: () => void;
  onOpenDocs: () => void;
  onRecheck: () => void;
  children?: React.ReactNode;
}

export function ProviderRow({
  provider,
  meta,
  connected,
  isActive,
  isExpanded,
  isSaving,
  isTesting,
  providerModel,
  ollamaModels,
  expandedModels,
  expandedModelsLoading,
  expandedModelsCached,
  expandedModelsError,
  loadingOllama,
  pulling,
  apiKeyInput,
  showKey,
  baseUrlInput,
  configuredBaseUrl,
  onToggleExpand,
  onTestKey,
  onRemoveKey,
  onSelectModel,
  onPullOllama,
  onSetActive,
  onApiKeyChange,
  onToggleShowKey,
  onBaseUrlChange,
  onSaveKey,
  onOpenDocs,
  onRecheck,
  children,
}: Props) {
  return (
    <div
      className={`rounded-xl border transition-all ${isExpanded ? 'border-foreground/15 bg-foreground/[0.03]' : 'border-foreground/10 bg-foreground/[0.03]'}`}
    >
      {/* Row header.
          A `<button>` row would nest the "test key" `Button` below inside it —
          invalid DOM, and the inner click also fires this row's expand. React
          reports it as "<button> cannot contain a nested <button>"; it showed up
          in the log file once the renderer console bridge landed. Follows the
          repo's `role="button"` row pattern (see `NotificationBell`), which
          keeps keyboard activation without the nesting. */}
      <div
        role="button"
        tabIndex={0}
        onClick={onToggleExpand}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onToggleExpand();
          }
        }}
        className="flex w-full cursor-pointer items-center gap-3 px-4 py-3 text-left"
      >
        <Bot size={15} className={connected ? meta.color : 'text-foreground/25'} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-sm font-medium text-foreground/80">
            {meta.label}
            {isActive && connected && (
              <span className="rounded-full border border-brand/30 bg-brand/10 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                Active
              </span>
            )}
          </div>
          <div className="mt-0.5 text-[11px] text-foreground/35">{meta.description}</div>
        </div>
        {/* Status badge — local providers (Ollama / CLI agents) are detected, not keyed */}
        {meta.kind !== 'cloud' ? (
          connected ? (
            <span className="flex items-center gap-1 text-[10px] text-emerald-400/80">
              <CheckCircle2 size={10} /> {meta.kind === 'local-server' ? 'Running' : 'Detected'}
            </span>
          ) : (
            <span className="flex items-center gap-1 text-[10px] text-amber-400/60">
              <WifiOff size={10} /> Not detected
            </span>
          )
        ) : connected ? (
          <div className="flex items-center gap-2">
            <span className="flex items-center gap-1 text-[10px] text-emerald-400/80">
              <Key size={10} /> Connected
            </span>
            {onTestKey && (
              <Button
                variant="glass"
                disabled={isTesting}
                onClick={(e) => {
                  // The row is a `role="button"` that expands on click, so
                  // without this a "test key" press ALSO toggles the row —
                  // which is the behaviour the old nested-<button> markup had
                  // and that unnesting alone does NOT fix: the click still
                  // bubbles. Same guard `ApplicationRow` uses for its note chip.
                  e.stopPropagation();
                  void onTestKey();
                }}
                // Enter/Space on a focused <button> also fires a native click,
                // so the keydown must be stopped too — otherwise it reaches the
                // row's own Enter/Space handler, whose `preventDefault()` can
                // suppress this button's activation entirely.
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
                }}
                className="h-auto px-1.5 py-0.5 text-[10px]"
              >
                {isTesting ? <Loader2 size={9} className="animate-spin" /> : <RefreshCw size={9} />}
              </Button>
            )}
          </div>
        ) : (
          <span className="text-[10px] text-foreground/30">Not connected</span>
        )}
      </div>

      {/* Expanded config */}
      {isExpanded && (
        <div className="border-t border-foreground/10 px-4 pb-4 pt-3 space-y-3">
          {meta.kind === 'local-server' ? (
            <OllamaConfig
              connected={connected}
              models={ollamaModels}
              providerModel={providerModel}
              loading={loadingOllama}
              pulling={pulling}
              onPull={onPullOllama}
              onSelect={(m) => onSelectModel('ollama', m)}
              onSetActive={onSetActive}
              isActive={isActive}
              onDownloadOllama={onOpenDocs}
              onRecheck={onRecheck}
            >
              {children}
            </OllamaConfig>
          ) : meta.kind === 'cli-agent' ? (
            <CliAgentConfig
              provider={provider}
              connected={connected}
              expandedModels={expandedModels}
              providerModel={providerModel}
              onSelect={(m) => onSelectModel(provider, m)}
              onSetActive={onSetActive}
              isActive={isActive}
              onInstall={onOpenDocs}
              onRecheck={onRecheck}
            />
          ) : (
            <CloudProviderConfig
              provider={provider}
              meta={meta}
              connected={connected}
              isSaving={isSaving}
              providerModel={providerModel}
              expandedModels={expandedModels}
              expandedModelsLoading={expandedModelsLoading}
              expandedModelsCached={expandedModelsCached}
              expandedModelsError={expandedModelsError}
              apiKeyInput={apiKeyInput}
              showKey={showKey}
              baseUrlInput={baseUrlInput}
              configuredBaseUrl={configuredBaseUrl}
              onApiKeyChange={onApiKeyChange}
              onToggleShowKey={onToggleShowKey}
              onBaseUrlChange={onBaseUrlChange}
              onSaveKey={onSaveKey}
              onRemoveKey={onRemoveKey}
              onSelectModel={(model) => onSelectModel(provider, model)}
              onSetActive={onSetActive}
              isActive={isActive}
              onOpenDocs={onOpenDocs}
              onRecheck={onRecheck}
            />
          )}
        </div>
      )}
    </div>
  );
}
