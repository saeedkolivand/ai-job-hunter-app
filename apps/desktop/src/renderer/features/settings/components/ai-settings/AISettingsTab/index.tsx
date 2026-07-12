import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { GlassCard, transition } from '@ajh/ui';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useDebugMode } from '@/store/preferences-store';

import { ActiveProviderSwitcher } from '../ActiveProviderSwitcher';
import { CompanyResearchSettings } from '../CompanyResearchSettings';
import { EmbeddingsSettings } from '../EmbeddingsSettings';
import { ProviderDebugBadge } from '../ProviderDebugBadge';
import { ProviderRow } from '../ProviderRow';
import { SpendSettings } from '../SpendSettings';
import { LocalModelLimits } from './LocalModelLimits';
import { OllamaResourcesPanel } from './OllamaResourcesPanel';
import { useProviderKeys } from './useProviderKeys';

export function AISettingsTab() {
  const { t } = useTranslation();
  // Debug-only affordances (e.g. the routing badge) appear only when the
  // developer "debug mode" toggle is on — otherwise the toggle was cosmetic (H14).
  const debugMode = useDebugMode();
  const {
    activeProvider,
    setActiveProvider,
    connectedProviders,
    keyStatus,
    providerConfig,
    selectedOllamaModel,
    ollamaModels,
    loadingOllama,
    expanded,
    expandedModels,
    apiKeyInput,
    showKey,
    savingKey,
    testingKey,
    baseUrlInput,
    pulling,
    handleSelectModel,
    handleSaveKey,
    handleTestKey,
    handleRemoveKey,
    handlePullOllama,
    toggleExpand,
    setApiKeyInput,
    toggleShowKey,
    setBaseUrlInput,
    recheck,
    openDocs,
  } = useProviderKeys();

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="space-y-4"
    >
      {/* Active provider switcher */}
      <ActiveProviderSwitcher
        providers={connectedProviders}
        meta={PROVIDERS}
        activeProvider={activeProvider}
        onSetActive={setActiveProvider}
      />

      {/* Routing debug — shows exactly where AI requests will go. Gated behind
          the developer debug-mode toggle (Settings → Developer). */}
      {debugMode && <ProviderDebugBadge />}

      {/* Provider list */}
      <GlassCard>
        <div className="mb-3 text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('settings.aiProvider.title')}
        </div>
        <div className="space-y-2">
          {PROVIDER_ORDER.map((p) => {
            const m = PROVIDERS[p];
            const connected = keyStatus[p] ?? false;

            return (
              <ProviderRow
                key={p}
                provider={p}
                meta={m}
                connected={connected}
                isActive={p === activeProvider}
                isExpanded={expanded === p}
                isSaving={savingKey === p}
                isTesting={testingKey === p}
                providerModel={providerConfig?.providers?.[p]?.model ?? ''}
                ollamaModels={ollamaModels}
                expandedModels={expandedModels}
                loadingOllama={loadingOllama}
                pulling={pulling}
                apiKeyInput={apiKeyInput}
                showKey={showKey}
                baseUrlInput={baseUrlInput}
                onToggleExpand={() => toggleExpand(p)}
                onTestKey={() => void handleTestKey(p)}
                onRemoveKey={() => void handleRemoveKey(p)}
                onSelectModel={handleSelectModel}
                onPullOllama={handlePullOllama}
                onSetActive={() => setActiveProvider(p)}
                onApiKeyChange={setApiKeyInput}
                onToggleShowKey={toggleShowKey}
                onBaseUrlChange={setBaseUrlInput}
                onSaveKey={() => void handleSaveKey(p)}
                onOpenDocs={() => openDocs(m.docsUrl)}
                onRecheck={recheck}
              >
                {p === 'ollama' && connected && (
                  <>
                    <OllamaResourcesPanel selectedModel={selectedOllamaModel} />
                    <LocalModelLimits selectedModel={selectedOllamaModel} />
                  </>
                )}
              </ProviderRow>
            );
          })}
        </div>
      </GlassCard>

      {/* Embeddings — provider/model for matching & search, with re-indexing */}
      <div data-settings-anchor="ai-embeddings">
        <EmbeddingsSettings />
      </div>

      {/* Company research — provider-native web search (Ollama needs the free key) */}
      <div data-settings-anchor="ai-company-research">
        <CompanyResearchSettings />
      </div>

      {/* AI spend — today's real token totals + a rate-table cost estimate */}
      <div data-settings-anchor="ai-spend">
        <SpendSettings />
      </div>
    </motion.div>
  );
}
