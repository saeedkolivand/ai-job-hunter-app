import { Bot, CheckCircle2, Eye, EyeOff, Key, Loader2, RefreshCw } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, transition, useNotification } from '@ajh/ui';

import {
  useHasProviderKey,
  useOpenExternal,
  useSetProviderKey,
  useTestProviderKey,
} from '@/services';
import type { AiProvider } from '@/store/preferences-schema';
import { usePreferencesStore } from '@/store/preferences-store';

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

const CLOUD_DEFAULT_MODELS: Record<string, string> = {
  'ollama-cloud': 'gpt-oss:120b',
  openai: 'gpt-4o',
  anthropic: 'claude-sonnet-4-6',
  gemini: 'gemini-2.0-flash',
  'openai-compatible': 'gpt-4o',
};

interface CloudProviderPanelProps {
  selectedProvider: AiProvider;
  onProviderChange: (provider: AiProvider) => void;
}

export function CloudProviderPanel({
  selectedProvider,
  onProviderChange,
}: CloudProviderPanelProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();
  const testProviderKey = useTestProviderKey();
  const setAiProviderConfig = usePreferencesStore((s) => s.setAiProviderConfig);

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);

  const cloudMeta = CLOUD_PROVIDERS.find((p) => p.id === selectedProvider) ?? CLOUD_PROVIDERS[0];
  const { data: hasKeyData } = useHasProviderKey(selectedProvider);
  const hasKey = hasKeyData?.has ?? false;

  const handleSaveKey = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider: selectedProvider, apiKey: apiKey.trim() });
      setApiKey('');
      setAiProviderConfig({
        activeProvider: selectedProvider,
        providers: { [selectedProvider]: { model: CLOUD_DEFAULT_MODELS[selectedProvider] ?? '' } },
      });
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
                : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20'
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
        <div className="flex items-center justify-between gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5">
          <div className="flex items-center gap-2">
            <Key size={13} className="text-emerald-400" />
            <span className="text-sm text-emerald-300/80">{t('onboarding.ai.apiKeyStored')}</span>
          </div>
          <Button
            variant="glass"
            disabled={testing}
            onClick={() => void handleTestKey()}
            className="h-auto px-2 py-1 text-xs"
          >
            {testing ? (
              <Loader2 size={11} className="animate-spin" />
            ) : (
              <>
                <RefreshCw size={11} className="mr-1" />
                Test
              </>
            )}
          </Button>
        </div>
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
