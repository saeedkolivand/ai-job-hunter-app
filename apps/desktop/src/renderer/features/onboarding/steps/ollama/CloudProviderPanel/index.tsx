import { Bot, CheckCircle2, Eye, EyeOff, Loader2, RefreshCw } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Alert, Button, Dropdown, Input, transition, useNotification } from '@ajh/ui';

import { sortModelsNewestFirst } from '@/lib/ai-providers/model-sort';
import {
  useHasProviderKey,
  useListProviderModels,
  useOpenExternal,
  useSetProviderKey,
  useTestProviderKey,
} from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

interface CloudProvider {
  id: AiProvider;
  label: string;
  placeholder: string;
  docsUrl: string;
  color: string;
}

const CLOUD_PROVIDERS: CloudProvider[] = [
  {
    id: 'ollama-cloud',
    label: 'Ollama Cloud',
    placeholder: 'Ollama API key…',
    docsUrl: 'https://ollama.com/settings/keys',
    color: 'text-emerald-400',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    placeholder: 'sk-...',
    docsUrl: 'https://platform.openai.com/api-keys',
    color: 'text-green-400',
  },
  {
    id: 'anthropic',
    label: 'Anthropic (Claude)',
    placeholder: 'sk-ant-...',
    docsUrl: 'https://console.anthropic.com/settings/keys',
    color: 'text-orange-400',
  },
  {
    id: 'gemini',
    label: 'Google Gemini',
    placeholder: 'AIza...',
    docsUrl: 'https://aistudio.google.com/app/apikey',
    color: 'text-blue-400',
  },
  {
    id: 'openai-compatible',
    label: 'OpenAI-Compatible',
    placeholder: 'API key...',
    docsUrl: 'https://platform.openai.com/docs/api-reference',
    color: 'text-purple-400',
  },
];

interface CloudProviderPanelProps {
  selectedProvider: AiProvider;
  onProviderChange: (provider: AiProvider) => void;
  /** Model chosen from the live list — empty until the user picks one. No id is
   *  ever pre-selected: a hardcoded onboarding default is exactly the defect
   *  class this step now avoids (a shut-down `gemini-2.0-flash` shipped as the
   *  Gemini default once already). */
  selectedModel: string;
  onModelSelect: (model: string) => void;
}

export function CloudProviderPanel({
  selectedProvider,
  onProviderChange,
  selectedModel,
  onModelSelect,
}: CloudProviderPanelProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();
  const testProviderKey = useTestProviderKey();

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  const cloudMeta = CLOUD_PROVIDERS.find((p) => p.id === selectedProvider) ?? CLOUD_PROVIDERS[0];
  const { data: hasKeyData } = useHasProviderKey(selectedProvider);
  const hasKey = hasKeyData?.has ?? false;

  // Saving a key (or switching to an already-keyed provider) swaps this whole
  // section from the key-input tree to the Alert + model-picker tree,
  // unmounting whatever was focused. Before, that was cosmetic (a default
  // model was applied automatically); now the swapped-in content is the
  // MANDATORY model picker, so a keyboard user must land somewhere in it —
  // move focus to the heading rather than dropping to <body>. Only fires on
  // the actual false→true transition (`wasHasKey` seeds from the CURRENT
  // value on first render), never on mount with a key already stored.
  const chooseModelHeadingRef = useRef<HTMLParagraphElement>(null);
  const wasHasKey = useRef(hasKey);
  useEffect(() => {
    if (!wasHasKey.current && hasKey) {
      chooseModelHeadingRef.current?.focus();
    }
    wasHasKey.current = hasKey;
  }, [hasKey]);

  // Model choice is deferred until the key is entered and VERIFIED — the live
  // list IS the verification (a successful fetch means the key works). This
  // is a VERIFY call (`purpose: 'verify'`), not a display one: the local
  // cache is keyed by provider + base URL with NO credential identity, so a
  // cache hit from a PRIOR key would let a newly-entered wrong/revoked key
  // pass verification on a list that proves nothing about it — Continue is
  // supposed to mean the provider works. `useListProviderModels` is still the
  // same hook the model picker and Settings use (so saving the key, which
  // invalidates its query, is the only fetch — not a second one); only the
  // cache-fallback behavior differs by `purpose`.
  const modelsQuery = useListProviderModels(selectedProvider, hasKey, undefined, 'verify');
  const models = modelsQuery.data?.models ?? [];
  const modelsErrorMessage = modelsQuery.isError
    ? modelsQuery.error instanceof Error
      ? modelsQuery.error.message
      : String(modelsQuery.error)
    : undefined;

  const handleSaveKey = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider: selectedProvider, apiKey: apiKey.trim() });
      setApiKey('');
      notify.success({ message: `${cloudMeta?.label ?? selectedProvider} API key saved.` });
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Failed to save key.' });
    } finally {
      setSaving(false);
    }
  };

  const handleTestKey = async () => {
    if (!hasKey) return;
    setTesting(true);
    try {
      const result = await testProviderKey.mutateAsync({ provider: selectedProvider });
      if (result.success) {
        notify.success({ message: `${cloudMeta?.label ?? selectedProvider} API key is valid!` });
      } else {
        notify.error({ message: `API key test failed: ${result.error ?? 'Unknown error'}` });
      }
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Failed to test key.' });
    } finally {
      setTesting(false);
    }
  };

  return (
    <motion.div
      key="cloud-panel"
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -20, scale: 0.95 }}
      transition={transition.normal}
      className="mb-6 space-y-4"
    >
      {/* Provider selector */}
      <div className="space-y-2">
        {CLOUD_PROVIDERS.map((p) => (
          <Button
            key={p.id}
            variant="unstyled"
            onClick={() => onProviderChange(p.id)}
            className={`flex w-full items-center gap-3 rounded-xl border px-4 py-2.5 text-left transition-all duration-150 ${
              selectedProvider === p.id
                ? 'border-brand/40 bg-brand/10'
                : 'border-[var(--border-clear)] bg-card hover:bg-muted'
            }`}
          >
            <Bot size={14} className={selectedProvider === p.id ? p.color : 'text-foreground/30'} />
            <span
              className={`text-sm font-medium ${
                selectedProvider === p.id ? 'text-foreground/90' : 'text-foreground/60'
              }`}
            >
              {p.label}
            </span>
            {selectedProvider === p.id && hasKey && (
              <CheckCircle2 size={12} className="ml-auto text-emerald-400" />
            )}
          </Button>
        ))}
      </div>

      {/* API key input */}
      {hasKey ? (
        <>
          <Alert
            type="success"
            showIcon
            message={t('onboarding.ai.apiKeyStored')}
            action={
              <Button
                variant="glass"
                disabled={testing}
                onClick={() => void handleTestKey()}
                className="h-auto px-2 py-1 text-xs"
                aria-label={testing ? t('onboarding.ai.testingKey') : undefined}
              >
                {testing ? (
                  <Loader2 size={11} className="animate-spin" aria-hidden="true" />
                ) : (
                  <>
                    <RefreshCw size={11} className="mr-1" />
                    {t('onboarding.ai.testKey')}
                  </>
                )}
              </Button>
            }
          />

          {/* Model picker — deferred until the key is verified via the live
              fetch itself (the same fetch invalidated by saving the key, so
              this is the ONE round trip, not a second verification call).
              Reuses the picker's own three-state vocabulary rather than
              inventing new copy: no key (n/a here — key is already stored),
              cache served, or the real failure message. Cached/empty are the
              quiet inline treatment (matches ModelSelector/Settings) — an
              amber `Alert` was too loud for "usually completely fine" facts;
              only a real failure gets the assertive `Alert type="error"`. */}
          <div className="space-y-2">
            <p
              ref={chooseModelHeadingRef}
              tabIndex={-1}
              className="rounded text-xs font-semibold uppercase tracking-widest text-foreground/55 focus:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1"
            >
              {t('onboarding.ai.chooseModel')}
            </p>
            {modelsQuery.isLoading ? (
              <div
                role="status"
                aria-live="polite"
                className="flex items-center gap-2 text-xs text-foreground/40"
              >
                <Loader2 size={13} className="animate-spin" />
                {t('settings.aiModel.loading')}
              </div>
            ) : modelsQuery.isError ? (
              <Alert
                type="error"
                showIcon
                message={t('models.cloud.fetchFailed', { message: modelsErrorMessage })}
              />
            ) : models.length === 0 ? (
              <div role="status" aria-live="polite" className="text-[10px] text-foreground/40">
                <p>{t('settings.aiModel.emptyTitle')}</p>
                <p className="mt-0.5">{t('settings.aiModel.emptyDescription')}</p>
              </div>
            ) : (
              <>
                {modelsQuery.data?.cached && (
                  <p role="status" aria-live="polite" className="text-[10px] text-foreground/40">
                    {t('models.cloud.cachedList')}
                  </p>
                )}
                <Dropdown
                  options={sortModelsNewestFirst(models).map((m) => ({
                    value: m.name,
                    label: m.displayName ?? m.name,
                  }))}
                  value={selectedModel}
                  onChange={onModelSelect}
                  placeholder={t('onboarding.ai.selectModelPlaceholder')}
                />
              </>
            )}
          </div>
        </>
      ) : (
        <div className="space-y-2">
          <p className="text-xs text-foreground/35">
            {t('onboarding.ai.getApiKeyAt')}{' '}
            <Button
              variant="unstyled"
              onClick={() => void openExternal.mutateAsync(cloudMeta?.docsUrl ?? '')}
              className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
            >
              {(cloudMeta?.docsUrl ?? '').replace('https://', '')}
            </Button>
          </p>
          <div className="flex flex-col gap-2">
            <div className="relative">
              <Input
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void handleSaveKey()}
                placeholder={cloudMeta?.placeholder ?? '…'}
                className="w-full pr-9 text-sm"
              />
              <Button
                variant="unstyled"
                onClick={() => setShowKey((v) => !v)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </Button>
            </div>
            <div className="flex justify-end">
              <Button
                variant="glass"
                disabled={!apiKey.trim() || saving}
                onClick={() => void handleSaveKey()}
                className={apiKey.trim() && !saving ? 'ring-1 ring-brand/20' : ''}
              >
                {saving ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  t('onboarding.ai.saveKey')
                )}
              </Button>
            </div>
          </div>
        </div>
      )}
    </motion.div>
  );
}
