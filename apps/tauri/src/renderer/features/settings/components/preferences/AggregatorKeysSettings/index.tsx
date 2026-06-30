import { AlertCircle, Check, Eye, EyeOff, Key, Loader2, Search, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, Input, SettingsSection, Switch, useNotification } from '@ajh/ui';

import {
  type ScrapingSettings,
  useHasProviderKey,
  useOpenExternal,
  useRemoveProviderKey,
  useScrapingSettings,
  useSetProviderKey,
  useUpdateScrapingSettings,
} from '@/services';

const ADZUNA_DOCS_URL = 'https://developer.adzuna.com';

// ── Credential key field (reused for every provider) ─────────────────────────

interface KeyFieldProps {
  slot: string;
  labelKey: string;
  placeholderKey: string;
  connectedKey: string;
  removeConfirmTitleKey: string;
  removeConfirmDescKey: string;
}

function AggregatorKeyField({
  slot,
  labelKey,
  placeholderKey,
  connectedKey,
  removeConfirmTitleKey,
  removeConfirmDescKey,
}: KeyFieldProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();
  const { data: keyData } = useHasProviderKey(slot);
  const connected = keyData?.has ?? false;

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const handleSave = async () => {
    if (saving || setProviderKey.isPending || !apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider: slot, apiKey: apiKey.trim() });
      setApiKey('');
      notify.success({ message: t('settings.aggregatorKeys.saved') });
    } catch {
      notify.error({ message: t('settings.aggregatorKeys.saveError') });
    } finally {
      setSaving(false);
    }
  };

  const handleRemove = async () => {
    if (removeProviderKey.isPending) return;
    setConfirmRemove(false);
    try {
      await removeProviderKey.mutateAsync({ provider: slot });
      notify.success({ message: t('settings.aggregatorKeys.removed') });
    } catch {
      notify.error({ message: t('settings.aggregatorKeys.removeError') });
    }
  };

  return (
    <div className="space-y-1.5">
      <div className="text-xs font-semibold uppercase tracking-[0.12em] text-foreground/55">
        {t(labelKey)}
      </div>

      {connected ? (
        <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
          <div className="flex items-center gap-2 text-sm text-emerald-300/80">
            <Key size={12} /> {t(connectedKey)}
          </div>
          <Button
            variant="ghost"
            className="text-xs text-red-400/60 hover:text-red-400"
            onClick={() => setConfirmRemove(true)}
          >
            <Trash2 size={11} /> {t('settings.aggregatorKeys.remove')}
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          <div className="relative">
            <Input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void handleSave()}
              placeholder={t(placeholderKey)}
              className="w-full pr-9 text-sm"
            />
            <Button
              variant="unstyled"
              aria-label={t(
                showKey ? 'settings.aiProvider.hideKey' : 'settings.aiProvider.showKey'
              )}
              onClick={() => setShowKey((v) => !v)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </Button>
          </div>
          <div className="flex justify-end">
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
                  <Check size={12} /> {t('settings.aggregatorKeys.save')}
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
        title={t(removeConfirmTitleKey)}
        description={t(removeConfirmDescKey)}
        confirmText={t('settings.aggregatorKeys.remove')}
        variant="danger"
        isConfirming={removeProviderKey.isPending}
      />
    </div>
  );
}

// ── Apify LinkedIn non-secret settings (toggle + actor override) ─────────────

interface ApifyLinkedinSectionProps {
  settings: ScrapingSettings;
}

function ApifyLinkedinSection({ settings }: ApifyLinkedinSectionProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const updateSettings = useUpdateScrapingSettings();
  const storedActorId = settings.apifyLinkedinActorId ?? '';
  const [actorId, setActorId] = useState(storedActorId);
  const [saving, setSaving] = useState(false);
  const actorIdDirty = actorId !== storedActorId;

  const handleToggle = async (enabled: boolean) => {
    try {
      await updateSettings.mutateAsync({ apifyLinkedinEnabled: enabled });
    } catch {
      notify.error({ message: t('settings.aggregatorKeys.apifyLinkedin.saveError') });
    }
  };

  const handleSaveActorId = async () => {
    if (saving || updateSettings.isPending) return;
    setSaving(true);
    try {
      await updateSettings.mutateAsync({ apifyLinkedinActorId: actorId });
      notify.success({ message: t('settings.aggregatorKeys.apifyLinkedin.saved') });
    } catch {
      notify.error({ message: t('settings.aggregatorKeys.apifyLinkedin.saveError') });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="text-xs font-semibold uppercase tracking-[0.12em] text-foreground/55">
        {t('settings.aggregatorKeys.apifyLinkedin.sectionLabel')}
      </div>

      {/* Toggle row — mirrors the Switch pattern in PrivacySettingsTab */}
      <div className="flex items-start gap-4 rounded-xl border border-foreground/10 px-4 py-3">
        <div className="min-w-0 flex-1">
          <div className="text-sm font-medium text-foreground/85">
            {t('settings.aggregatorKeys.apifyLinkedin.enabledLabel')}
          </div>
          <div className="mt-0.5 text-xs leading-snug text-foreground/45">
            {t('settings.aggregatorKeys.apifyLinkedin.enabledDescription')}
          </div>
        </div>
        <Switch
          checked={settings.apifyLinkedinEnabled}
          onCheckedChange={(v) => void handleToggle(v)}
          aria-label={t('settings.aggregatorKeys.apifyLinkedin.enabledLabel')}
        />
      </div>

      {/* Cost / latency notice — role="note" so screen readers announce it */}
      <div
        role="note"
        className="flex gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-xs text-amber-300/80"
      >
        <AlertCircle size={12} className="mt-0.5 shrink-0" />
        <span>{t('settings.aggregatorKeys.apifyLinkedin.costWarning')}</span>
      </div>

      {/* Actor ID override */}
      <div className="space-y-1.5">
        <div className="text-xs font-semibold uppercase tracking-[0.12em] text-foreground/55">
          {t('settings.aggregatorKeys.apifyLinkedin.actorIdLabel')}
        </div>
        <div className="flex gap-2">
          <Input
            value={actorId}
            onChange={(e) => setActorId(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void handleSaveActorId()}
            placeholder={t('settings.aggregatorKeys.apifyLinkedin.actorIdPlaceholder')}
            aria-label={t('settings.aggregatorKeys.apifyLinkedin.actorIdLabel')}
            className="flex-1 text-sm"
          />
          <Button
            variant="glass"
            disabled={saving || updateSettings.isPending || !actorIdDirty}
            onClick={() => void handleSaveActorId()}
          >
            {saving ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <>
                <Check size={12} /> {t('settings.aggregatorKeys.save')}
              </>
            )}
          </Button>
        </div>
        <p className="text-xs text-foreground/40">
          {t('settings.aggregatorKeys.apifyLinkedin.actorIdHelper')}
        </p>
      </div>
    </div>
  );
}

// ── Public section component ──────────────────────────────────────────────────

export function AggregatorKeysSettings() {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const { data: scrapingSettings } = useScrapingSettings();

  return (
    <SettingsSection icon={Search} label={t('settings.aggregatorKeys.title')}>
      <p className="mb-3 text-xs leading-relaxed text-foreground/50">
        {t('settings.aggregatorKeys.description')}{' '}
        <Button
          variant="unstyled"
          onClick={() => void openExternal.mutateAsync(ADZUNA_DOCS_URL)}
          className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
        >
          developer.adzuna.com
        </Button>{' '}
        {t('settings.aggregatorKeys.descriptionSuffix')}
      </p>

      <div className="space-y-4">
        <AggregatorKeyField
          slot={PROVIDER_SLOTS.adzunaAppId}
          labelKey="settings.aggregatorKeys.adzunaAppId.label"
          placeholderKey="settings.aggregatorKeys.adzunaAppId.placeholder"
          connectedKey="settings.aggregatorKeys.adzunaAppId.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.adzunaAppId.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.adzunaAppId.removeConfirmDesc"
        />

        <AggregatorKeyField
          slot={PROVIDER_SLOTS.adzunaAppKey}
          labelKey="settings.aggregatorKeys.adzunaAppKey.label"
          placeholderKey="settings.aggregatorKeys.adzunaAppKey.placeholder"
          connectedKey="settings.aggregatorKeys.adzunaAppKey.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.adzunaAppKey.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.adzunaAppKey.removeConfirmDesc"
        />

        <AggregatorKeyField
          slot={PROVIDER_SLOTS.jsearchKey}
          labelKey="settings.aggregatorKeys.jsearchKey.label"
          placeholderKey="settings.aggregatorKeys.jsearchKey.placeholder"
          connectedKey="settings.aggregatorKeys.jsearchKey.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.jsearchKey.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.jsearchKey.removeConfirmDesc"
        />

        {/* Apify API token — credential slot for the LinkedIn (Apify) provider */}
        <AggregatorKeyField
          slot={PROVIDER_SLOTS.apifyToken}
          labelKey="settings.aggregatorKeys.apifyToken.label"
          placeholderKey="settings.aggregatorKeys.apifyToken.placeholder"
          connectedKey="settings.aggregatorKeys.apifyToken.connected"
          removeConfirmTitleKey="settings.aggregatorKeys.apifyToken.removeConfirmTitle"
          removeConfirmDescKey="settings.aggregatorKeys.apifyToken.removeConfirmDesc"
        />

        {/* Non-secret Apify LinkedIn settings (toggle + actor override + cost warning) */}
        {scrapingSettings && (
          <div className="border-t border-foreground/10 pt-2">
            <ApifyLinkedinSection settings={scrapingSettings} />
          </div>
        )}
      </div>
    </SettingsSection>
  );
}
