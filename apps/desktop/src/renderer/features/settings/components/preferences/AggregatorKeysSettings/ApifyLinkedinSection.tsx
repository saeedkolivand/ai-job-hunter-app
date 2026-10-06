import { AlertCircle, Check, Loader2 } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, Switch, useNotification } from '@ajh/ui';

import { type ScrapingSettings, useUpdateScrapingSettings } from '@/services';

// ── Apify LinkedIn non-secret settings (toggle + actor override) ─────────────

interface ApifyLinkedinSectionProps {
  settings: ScrapingSettings;
}

export function ApifyLinkedinSection({ settings }: ApifyLinkedinSectionProps) {
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
