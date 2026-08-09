import { Check, Eye, EyeOff, Key, Loader2, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, Input, useNotification } from '@ajh/ui';

import {
  useHasProviderKey,
  useOpenExternal,
  useRemoveProviderKey,
  useSetProviderKey,
} from '@/services';

export interface SearchKeyFieldProps {
  /** Credential slot (`ai:<provider>` in the OS keychain) — `ollama-cloud` or `exa`. */
  provider: string;
  /** Where the user gets this key. Rendered as the visible link text too. */
  keysUrl: string;
  /** Shown in place of the field once a key is stored. */
  connectedLabel: string;
  getKeyLabel: string;
  placeholder: string;
  /** One-line hint under the field (what else the key unlocks, or what it costs). */
  note: string;
  savedMessage: string;
  removedMessage: string;
  removeConfirmTitle: string;
  removeConfirmDescription: string;
}

/**
 * One stored search-backend key: paste-to-save, masked reveal, remove-with-confirm.
 *
 * Extracted from `CompanyResearchSettings` when a second key appeared. The two
 * differ only in labels and which credential slot they write, so parameterising
 * beat a copy — and `useSetProviderKey`/`useRemoveProviderKey` already accept an
 * arbitrary slot name, so neither key needed new IPC.
 */
export function SearchKeyField({
  provider,
  keysUrl,
  connectedLabel,
  getKeyLabel,
  placeholder,
  note,
  savedMessage,
  removedMessage,
  removeConfirmTitle,
  removeConfirmDescription,
}: SearchKeyFieldProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();
  const { data: keyData } = useHasProviderKey(provider);
  const connected = keyData?.has ?? false;

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const handleSave = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider, apiKey: apiKey.trim() });
      setApiKey('');
      notify.success({ message: savedMessage });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('settings.companyResearch.saveError'),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleRemove = async () => {
    setConfirmRemove(false);
    try {
      await removeProviderKey.mutateAsync({ provider });
      notify.success({ message: removedMessage });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('settings.companyResearch.saveError'),
      });
    }
  };

  return (
    <>
      {connected ? (
        <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
          <div className="flex items-center gap-2 text-sm text-emerald-300/80">
            <Key size={12} /> {connectedLabel}
          </div>
          <Button
            variant="ghost"
            className="text-xs text-red-400/60 hover:text-red-400"
            onClick={() => setConfirmRemove(true)}
          >
            <Trash2 size={11} /> {t('settings.companyResearch.remove')}
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          <p className="text-xs text-foreground/40">
            {getKeyLabel}{' '}
            <Button
              variant="unstyled"
              onClick={() => void openExternal.mutateAsync(keysUrl)}
              className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
            >
              {keysUrl.replace('https://', '')}
            </Button>
          </p>
          <div className="relative">
            <Input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void handleSave()}
              placeholder={placeholder}
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
            <span className="text-[10px] text-foreground/30">{note}</span>
            <Button
              variant="glass"
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

      <ConfirmModal
        open={confirmRemove}
        onClose={() => setConfirmRemove(false)}
        onConfirm={() => void handleRemove()}
        title={removeConfirmTitle}
        description={removeConfirmDescription}
        confirmText={t('settings.companyResearch.remove')}
        variant="danger"
        isConfirming={removeProviderKey.isPending}
      />
    </>
  );
}
