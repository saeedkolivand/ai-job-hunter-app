import { AlertTriangle, Check, RefreshCw, X } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, SettingsSection, useNotification } from '@ajh/ui';

import { useConfigureSync, useDisableSync, useSyncNow, useSyncStatus } from '@/services';

function Spinner() {
  return (
    <span className="h-3 w-3 motion-safe:animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
  );
}

/**
 * Optional self-hosted sync (ADR-051). Off by default: the user enters the
 * URL and token of a server they run themselves. `configure` checks the
 * server, stores the token in the OS keychain, turns sync on and runs once;
 * the backend then syncs after launch and every 15 minutes. The token is
 * write-only: it is cleared from the form once sent and never shown again.
 */
export function SyncSection() {
  const { t } = useTranslation();
  const notify = useNotification();

  const statusQuery = useSyncStatus();
  const status = statusQuery.data;
  const configure = useConfigureSync();
  const disable = useDisableSync();
  const syncNow = useSyncNow();

  const [serverUrl, setServerUrl] = useState('');
  const [token, setToken] = useState('');

  const typedInsecure = serverUrl.trim().toLowerCase().startsWith('http://');

  const handleConnect = async () => {
    try {
      await configure.mutateAsync({ serverUrl: serverUrl.trim(), token });
    } catch {
      // Shown inline below via configure.isError.
    } finally {
      setToken('');
    }
  };

  const handleDisable = async () => {
    try {
      await disable.mutateAsync();
    } catch {
      notify.error({ message: t('settings.privacy.sync.disableFailed') });
    }
  };

  const handleSyncNow = async () => {
    try {
      await syncNow.mutateAsync();
    } catch {
      notify.error({ message: t('settings.privacy.sync.syncNowFailed') });
    }
  };

  const report = status?.lastReport;

  // Until the first status arrives, say nothing about on/off: showing "off"
  // (and the form) while sync is in fact on would invite re-configuring it.
  if (!status) {
    return (
      <SettingsSection icon={RefreshCw} label={t('settings.privacy.sync.title')}>
        {statusQuery.isError ? (
          <div className="flex items-center justify-between gap-3">
            <p className="text-xs text-red-400/80">{t('settings.privacy.sync.loadFailed')}</p>
            <Button variant="glass" onClick={() => void statusQuery.refetch()}>
              {t('settings.privacy.sync.retry')}
            </Button>
          </div>
        ) : (
          <p className="flex items-center gap-2 text-xs text-foreground/55">
            <Spinner />
            {t('settings.privacy.sync.loading')}
          </p>
        )}
      </SettingsSection>
    );
  }

  return (
    <SettingsSection icon={RefreshCw} label={t('settings.privacy.sync.title')}>
      <div className="space-y-4">
        <p className="text-xs text-foreground/55">{t('settings.privacy.sync.description')}</p>

        <div className="flex items-center justify-between gap-3">
          <span className="text-xs text-foreground/55">
            {t('settings.privacy.sync.statusLabel')}
          </span>
          {status.enabled ? (
            <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
              <Check size={9} strokeWidth={2.5} />
              {t('settings.privacy.sync.on')}
            </span>
          ) : (
            <span className="flex items-center gap-1 rounded-full bg-foreground/[0.06] px-2 py-0.5 text-[10px] font-medium text-foreground/45">
              <X size={9} strokeWidth={2.5} />
              {t('settings.privacy.sync.off')}
            </span>
          )}
        </div>

        {status.enabled ? (
          <>
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-foreground/55">
                {t('settings.privacy.sync.serverLabel')}
              </span>
              <span className="truncate text-xs text-foreground/80">{status.serverUrl}</span>
            </div>

            {status.insecure && (
              <p className="flex items-start gap-1.5 text-xs text-amber-400/90">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                {t('settings.privacy.sync.insecureWarning')}
              </p>
            )}

            <div className="space-y-1 border-t border-foreground/10 pt-3 text-xs text-foreground/55">
              <p>
                {status.lastSyncAt
                  ? t('settings.privacy.sync.lastSync', {
                      value: new Date(status.lastSyncAt).toLocaleString(),
                    })
                  : t('settings.privacy.sync.neverSynced')}
              </p>
              {report && (
                <p>
                  {t('settings.privacy.sync.lastReport', {
                    pulled: report.pulled,
                    applied: report.applied,
                    pushed: report.pushed,
                  })}
                </p>
              )}
              {report && report.held > 0 && (
                <p className="text-amber-400/90">
                  {t('settings.privacy.sync.held', { count: report.held })}
                </p>
              )}
              {status.lastError && (
                <p className="text-red-400/80">
                  {t('settings.privacy.sync.lastError', { message: status.lastError })}
                </p>
              )}
            </div>

            <div className="flex justify-end gap-2 border-t border-foreground/10 pt-3">
              <Button
                variant="danger"
                disabled={disable.isPending}
                onClick={() => void handleDisable()}
              >
                {disable.isPending && <Spinner />}
                {disable.isPending
                  ? t('settings.privacy.sync.disabling')
                  : t('settings.privacy.sync.disable')}
              </Button>
              <Button
                variant="glass"
                disabled={syncNow.isPending}
                onClick={() => void handleSyncNow()}
              >
                {syncNow.isPending && <Spinner />}
                {syncNow.isPending
                  ? t('settings.privacy.sync.syncing')
                  : t('settings.privacy.sync.syncNow')}
              </Button>
            </div>
          </>
        ) : (
          <>
            <div className="space-y-1.5">
              <label className="text-xs text-foreground/55" htmlFor="sync-server-url">
                {t('settings.privacy.sync.serverUrlLabel')}
              </label>
              <Input
                id="sync-server-url"
                type="url"
                autoComplete="off"
                value={serverUrl}
                onChange={(e) => {
                  setServerUrl(e.target.value);
                  configure.reset();
                }}
                placeholder={t('settings.privacy.sync.serverUrlPlaceholder')}
                className="w-full text-sm"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-xs text-foreground/55" htmlFor="sync-token">
                {t('settings.privacy.sync.tokenLabel')}
              </label>
              <Input
                id="sync-token"
                type="password"
                autoComplete="off"
                value={token}
                onChange={(e) => {
                  setToken(e.target.value);
                  configure.reset();
                }}
                placeholder={t('settings.privacy.sync.tokenPlaceholder')}
                className="w-full text-sm"
              />
            </div>

            {typedInsecure && (
              <p className="flex items-start gap-1.5 text-xs text-amber-400/90">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                {t('settings.privacy.sync.insecureWarning')}
              </p>
            )}

            {configure.isError && (
              <p className="text-xs text-red-400/80">{t('settings.privacy.sync.connectFailed')}</p>
            )}

            <div className="flex justify-end">
              <Button
                variant="glass"
                disabled={!serverUrl.trim() || !token.trim() || configure.isPending}
                onClick={() => void handleConnect()}
              >
                {configure.isPending ? (
                  <>
                    <Spinner />
                    {t('settings.privacy.sync.connecting')}
                  </>
                ) : (
                  t('settings.privacy.sync.connect')
                )}
              </Button>
            </div>
          </>
        )}

        <p className="border-t border-foreground/10 pt-3 text-[11px] leading-snug text-foreground/40">
          {t('settings.privacy.sync.disclosure')}
        </p>
        <p className="text-[11px] leading-snug text-foreground/40">
          {t('settings.privacy.sync.scopeNote')}
        </p>
      </div>
    </SettingsSection>
  );
}
