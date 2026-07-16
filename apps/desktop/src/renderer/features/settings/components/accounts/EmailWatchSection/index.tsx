import { Check, Mail, X } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, SettingsSection, Switch, useNotification } from '@ajh/ui';

import {
  useConnectEmailWatch,
  useDisconnectEmailWatch,
  useEmailWatchCheckNow,
  useEmailWatchStatus,
  useOpenExternal,
  useSetEmailWatchEnabled,
} from '@/services';

const GOOGLE_APP_PASSWORD_URL = 'https://myaccount.google.com/apppasswords';

function Spinner() {
  return (
    <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
  );
}

/**
 * Email-confirmation watching (Task #23, auto-track Layer C) — PR A: connect
 * a Gmail app password, toggle the standing "watch" opt-in, and re-check the
 * connection on demand. No poller yet (PR B) — the enabled switch only saves
 * the preference the future automatic inbox check will read; "Check now" is
 * honest today (it re-validates the IMAP login, nothing more).
 */
export function EmailWatchSection() {
  const { t } = useTranslation();
  const notify = useNotification();

  const { data: status } = useEmailWatchStatus();
  const connect = useConnectEmailWatch();
  const disconnect = useDisconnectEmailWatch();
  const setEnabled = useSetEmailWatchEnabled();
  const checkNow = useEmailWatchCheckNow();
  const openExternal = useOpenExternal();

  const [address, setAddress] = useState('');
  const [appPassword, setAppPassword] = useState('');

  const connected = status?.connected ?? false;
  const enabled = status?.enabled ?? false;

  const handleConnect = async () => {
    try {
      await connect.mutateAsync({ address: address.trim(), appPassword });
    } catch {
      // Surfaced inline below via connect.isError — no raw error text.
    } finally {
      // Never keep the app password around once the mutation has fired —
      // success or failure. A failed login means a typo'd/expired password;
      // the user re-pastes a fresh one rather than editing the rejected one.
      setAppPassword('');
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnect.mutateAsync();
    } catch {
      notify.error({ message: t('settings.accounts.emailWatch.disconnectFailed') });
    }
  };

  const handleToggleEnabled = async (next: boolean) => {
    try {
      await setEnabled.mutateAsync(next);
    } catch {
      notify.error({ message: t('settings.accounts.emailWatch.toggleFailed') });
    }
  };

  const handleCheckNow = async () => {
    try {
      await checkNow.mutateAsync();
    } catch {
      notify.error({ message: t('settings.accounts.emailWatch.checkNowFailed') });
    }
  };

  return (
    <SettingsSection icon={Mail} label={t('settings.accounts.emailWatch.title')}>
      <div className="space-y-4">
        {connected ? (
          <>
            {/* Connection status */}
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-foreground/55">
                {t('settings.accounts.emailWatch.statusLabel')}
              </span>
              <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <Check size={9} strokeWidth={2.5} />
                {t('settings.accounts.emailWatch.connected')}
              </span>
            </div>

            {/* Connected address */}
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-foreground/55">
                {t('settings.accounts.emailWatch.addressLabel')}
              </span>
              <span className="text-xs text-foreground/80">{status?.address}</span>
            </div>

            {/* Standing watch opt-in — PR B's poller will read this */}
            <div className="space-y-2 border-t border-foreground/10 pt-3">
              <Switch
                checked={enabled}
                onCheckedChange={(next) => void handleToggleEnabled(next)}
                disabled={setEnabled.isPending}
                label={t('settings.accounts.emailWatch.watch.label')}
                description={t('settings.accounts.emailWatch.watch.description')}
              />
            </div>

            {/* Last check + manual re-check */}
            <div className="flex items-center justify-between gap-3 border-t border-foreground/10 pt-3">
              <span className="text-xs text-foreground/55">
                {status?.lastCheckAt
                  ? t('settings.accounts.emailWatch.lastCheck', {
                      value: new Date(status.lastCheckAt).toLocaleString(),
                    })
                  : t('settings.accounts.emailWatch.neverChecked')}
              </span>
              <Button
                variant="glass"
                disabled={checkNow.isPending}
                onClick={() => void handleCheckNow()}
              >
                {checkNow.isPending && <Spinner />}
                {checkNow.isPending
                  ? t('settings.accounts.emailWatch.checking')
                  : t('settings.accounts.emailWatch.checkNow')}
              </Button>
            </div>

            {/* Disconnect — low stakes (just removes the credential), inline per
                ExtensionBridgeSection's tone rather than a confirm modal. */}
            <div className="flex justify-end border-t border-foreground/10 pt-3">
              <Button
                variant="danger"
                disabled={disconnect.isPending}
                onClick={() => void handleDisconnect()}
              >
                {disconnect.isPending && <Spinner />}
                {disconnect.isPending
                  ? t('settings.accounts.emailWatch.disconnecting')
                  : t('settings.accounts.emailWatch.disconnect')}
              </Button>
            </div>
          </>
        ) : (
          <>
            {/* Disconnected status pill (mirrors ExtensionBridgeSection) */}
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs text-foreground/55">
                {t('settings.accounts.emailWatch.statusLabel')}
              </span>
              <span className="flex items-center gap-1 rounded-full bg-foreground/[0.06] px-2 py-0.5 text-[10px] font-medium text-foreground/45">
                <X size={9} strokeWidth={2.5} />
                {t('settings.accounts.emailWatch.disconnected')}
              </span>
            </div>

            <div className="space-y-1.5">
              <label className="text-xs text-foreground/55" htmlFor="email-watch-address">
                {t('settings.accounts.emailWatch.addressLabel')}
              </label>
              <Input
                id="email-watch-address"
                type="email"
                autoComplete="email"
                value={address}
                onChange={(e) => setAddress(e.target.value)}
                placeholder={t('settings.accounts.emailWatch.addressPlaceholder')}
                className="w-full text-sm"
              />
            </div>

            <div className="space-y-1.5">
              <label className="text-xs text-foreground/55" htmlFor="email-watch-app-password">
                {t('settings.accounts.emailWatch.appPasswordLabel')}
              </label>
              <Input
                id="email-watch-app-password"
                type="password"
                autoComplete="off"
                value={appPassword}
                onChange={(e) => setAppPassword(e.target.value)}
                placeholder={t('settings.accounts.emailWatch.appPasswordPlaceholder')}
                className="w-full text-sm"
              />
            </div>

            <p className="text-xs text-foreground/40">
              {t('settings.accounts.emailWatch.appPasswordHelp')}{' '}
              <Button
                variant="unstyled"
                onClick={() => void openExternal.mutateAsync(GOOGLE_APP_PASSWORD_URL)}
                className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
              >
                myaccount.google.com/apppasswords
              </Button>
            </p>

            {connect.isError && (
              <p className="text-xs text-red-400/80">
                {t('settings.accounts.emailWatch.connectFailed')}
              </p>
            )}

            <div className="flex justify-end">
              <Button
                variant="glass"
                disabled={!address.trim() || !appPassword.trim() || connect.isPending}
                onClick={() => void handleConnect()}
              >
                {connect.isPending ? (
                  <>
                    <Spinner />
                    {t('settings.accounts.emailWatch.connecting')}
                  </>
                ) : (
                  t('settings.accounts.emailWatch.connect')
                )}
              </Button>
            </div>
          </>
        )}

        {/* Disclosure — consent honesty, mirrors the extension section's tone */}
        <p className="border-t border-foreground/10 pt-3 text-[11px] leading-snug text-foreground/40">
          {t('settings.accounts.emailWatch.disclosure')}
        </p>
      </div>
    </SettingsSection>
  );
}
