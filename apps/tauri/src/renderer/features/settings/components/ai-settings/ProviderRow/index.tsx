import { Bot, CheckCircle2, Key, Loader2, RefreshCw, WifiOff } from 'lucide-react';

import { Button } from '@ajh/ui';

import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

import { CliAgentConfig } from '../CliAgentConfig';
import { CloudProviderConfig } from '../CloudProviderConfig';
import { OllamaConfig } from '../OllamaConfig';
import type { ProviderMeta } from '../provider-meta';

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
  expandedModels: Array<{ name: string }>;
  loadingOllama: boolean;
  pulling: string | null;
  apiKeyInput: string;
  showKey: boolean;
  baseUrlInput: string;
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
  loadingOllama,
  pulling,
  apiKeyInput,
  showKey,
  baseUrlInput,
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
      className={`rounded-xl border transition-all ${isExpanded ? 'border-white/15 bg-white/[0.03]' : 'border-white/[0.06] bg-white/[0.01]'}`}
    >
      {/* Row header */}
      <button
        onClick={onToggleExpand}
        className="flex w-full items-center gap-3 px-4 py-3 text-left"
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
                size="sm"
                disabled={isTesting}
                onClick={() => void onTestKey()}
                className="h-auto px-1.5 py-0.5 text-[10px]"
              >
                {isTesting ? <Loader2 size={9} className="animate-spin" /> : <RefreshCw size={9} />}
              </Button>
            )}
          </div>
        ) : (
          <span className="text-[10px] text-foreground/30">Not connected</span>
        )}
      </button>

      {/* Expanded config */}
      {isExpanded && (
        <div className="border-t border-white/[0.06] px-4 pb-4 pt-3 space-y-3">
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
              label={meta.label}
              connected={connected}
              models={meta.models}
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
              apiKeyInput={apiKeyInput}
              showKey={showKey}
              baseUrlInput={baseUrlInput}
              onApiKeyChange={onApiKeyChange}
              onToggleShowKey={onToggleShowKey}
              onBaseUrlChange={onBaseUrlChange}
              onSaveKey={onSaveKey}
              onRemoveKey={onRemoveKey}
              onSelectModel={(model) => onSelectModel(provider, model)}
              onSetActive={onSetActive}
              isActive={isActive}
              onOpenDocs={onOpenDocs}
            />
          )}
        </div>
      )}
    </div>
  );
}
