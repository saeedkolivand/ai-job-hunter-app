import { Check, Eye, EyeOff, Key, Loader2, Search, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, SettingsSection, useNotification } from '@ajh/ui';

import {
  useHasProviderKey,
  useOpenExternal,
  useRemoveProviderKey,
  useSetProviderKey,
} from '@/services';

const KEYS_URL = 'https://ollama.com/settings/keys';

/**
 * Company-research settings. Research runs on the **active AI provider's own**
 * web search — so it works automatically with cloud/CLI providers (their own
 * key). Only Ollama-family providers need the free Ollama account key (the same
 * `ai:ollama-cloud` key that unlocks Ollama Cloud), which this section manages.
 */
export function CompanyResearchSettings() {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();
  const { data: keyData } = useHasProviderKey('ollama-cloud');
  const connected = keyData?.has ?? false;

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider: 'ollama-cloud', apiKey: apiKey.trim() });
      setApiKey('');
      notify(t('settings.companyResearch.saved'), 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : t('settings.companyResearch.saveError'), 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleRemove = async () => {
    try {
      await removeProviderKey.mutateAsync({ provider: 'ollama-cloud' });
      notify(t('settings.companyResearch.removed'), 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : t('settings.companyResearch.saveError'), 'error');
    }
  };

  return (
    <SettingsSection icon={Search} label={t('settings.companyResearch.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.companyResearch.description')}
      </p>

      {connected ? (
        <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
          <div className="flex items-center gap-2 text-sm text-emerald-300/80">
            <Key size={12} /> {t('settings.companyResearch.connected')}
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="text-xs text-red-400/60 hover:text-red-400"
            onClick={() => void handleRemove()}
          >
            <Trash2 size={11} /> {t('settings.companyResearch.remove')}
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          <p className="text-xs text-foreground/40">
            {t('settings.companyResearch.getKeyAt')}{' '}
            <Button
              variant="unstyled"
              onClick={() => void openExternal.mutateAsync(KEYS_URL)}
              className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
            >
              {KEYS_URL.replace('https://', '')}
            </Button>
          </p>
          <div className="relative">
            <Input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void handleSave()}
              placeholder={t('settings.companyResearch.keyPlaceholder')}
              className="w-full pr-9 text-sm"
            />
            <Button
              variant="unstyled"
              onClick={() => setShowKey((v) => !v)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </Button>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-[10px] text-foreground/30">
              {t('settings.companyResearch.sameKeyNote')}
            </span>
            <Button
              variant="glass"
              size="sm"
              disabled={!apiKey.trim() || saving}
              onClick={() => void handleSave()}
              className={apiKey.trim() && !saving ? 'ring-1 ring-brand/20' : ''}
            >
              {saving ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <>
                  <Check size={12} /> {t('settings.companyResearch.save')}
                </>
              )}
            </Button>
          </div>
        </div>
      )}
    </SettingsSection>
  );
}
