import { Check, Copy, Puzzle, RotateCcw, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, ConfirmModal, Input, SettingsSection, Switch, useNotification } from '@ajh/ui';

import {
  useExtensionAutofillSetting,
  useExtensionBridgeStatus,
  useRegenerateExtensionToken,
  useSetExtensionAutofillSetting,
} from '@/services';
import { useUiStore } from '@/store/ui-store';

export function ExtensionBridgeSection() {
  const { t } = useTranslation();
  const notify = useNotification();
  const [confirmOpen, setConfirmOpen] = useState(false);

  const { data: status } = useExtensionBridgeStatus();
  const regenerate = useRegenerateExtensionToken();

  const { data: autofill } = useExtensionAutofillSetting();
  const setAutofill = useSetExtensionAutofillSetting();
  const autofillEnabled = autofill?.enabled ?? false;

  const handleToggleAutofill = async (next: boolean) => {
    try {
      await setAutofill.mutateAsync(next);
    } catch {
      notify.error({ message: t('settings.accounts.extension.autofill.toggleFailed') });
    }
  };

  const token = status?.token ?? '';
  const port = status?.port ?? null;
  const connected = status?.connected ?? false;

  // Deep-link focus: the `ajh://settings/extension` link routes here and sets a
  // one-shot ui-store flag. We consume it on mount / flip — scroll the token
  // field into view, focus it (which selects, so it's copy-ready), and give it a
  // ~1.4s ring glow. Mirrors the autopilot `focused`/`onFocusHandled` pattern.
  const extensionTokenFocus = useUiStore((s) => s.extensionTokenFocus);
  const setExtensionTokenFocus = useUiStore((s) => s.setExtensionTokenFocus);
  const tokenRef = useRef<HTMLInputElement>(null);
  const [highlight, setHighlight] = useState(false);

  useEffect(() => {
    if (!extensionTokenFocus) return;
    const el = tokenRef.current;
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      el.focus();
    }
    setHighlight(true);
    setExtensionTokenFocus(false);
    const timer = setTimeout(() => setHighlight(false), 1400);
    return () => clearTimeout(timer);
  }, [extensionTokenFocus, setExtensionTokenFocus]);

  const handleCopy = async () => {
    if (!token) return;
    try {
      await navigator.clipboard.writeText(token);
      notify.success({ message: t('settings.accounts.extension.copied') });
    } catch {
      notify.error({ message: t('settings.accounts.extension.copyFailed') });
    }
  };

  const handleRegenerate = async () => {
    setConfirmOpen(false);
    try {
      await regenerate.mutateAsync();
      notify.success({ message: t('settings.accounts.extension.regenerated') });
    } catch {
      notify.error({ message: t('settings.accounts.extension.regenerateFailed') });
    }
  };

  return (
    <>
      <SettingsSection icon={Puzzle} label={t('settings.accounts.extension.title')}>
        <div className="space-y-4">
          {/* Connection status */}
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-foreground/55">
              {t('settings.accounts.extension.statusLabel')}
            </span>
            {connected ? (
              <span className="flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <Check size={9} strokeWidth={2.5} />
                {t('settings.accounts.extension.connected')}
              </span>
            ) : (
              <span className="flex items-center gap-1 rounded-full bg-foreground/[0.06] px-2 py-0.5 text-[10px] font-medium text-foreground/45">
                <X size={9} strokeWidth={2.5} />
                {t('settings.accounts.extension.disconnected')}
              </span>
            )}
          </div>

          {/* Pairing token */}
          <div className="space-y-1.5">
            <label className="text-xs text-foreground/55" htmlFor="extension-pairing-token">
              {t('settings.accounts.extension.tokenLabel')}
            </label>
            <div className="flex items-center gap-2">
              <Input
                ref={tokenRef}
                id="extension-pairing-token"
                readOnly
                value={token}
                placeholder={t('settings.accounts.extension.tokenPlaceholder')}
                className={cn('flex-1 font-mono text-xs', highlight && 'ring-2 ring-brand')}
                onFocus={(e) => e.currentTarget.select()}
              />
              <Button
                variant="glass"
                disabled={!token}
                onClick={() => void handleCopy()}
                className="shrink-0"
              >
                <Copy size={11} />
                {t('settings.accounts.extension.copy')}
              </Button>
            </div>
          </div>

          {/* Port */}
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-foreground/55">
              {t('settings.accounts.extension.portLabel')}
            </span>
            <span className="font-mono text-xs text-foreground/80">
              {port ?? t('settings.accounts.extension.portUnavailable')}
            </span>
          </div>

          {/* Help + availability (published on the Chrome Web Store and Firefox AMO). */}
          <p className="text-xs leading-snug text-foreground/40">
            {t('settings.accounts.extension.help')} {t('settings.accounts.extension.availability')}
          </p>

          {/* Assisted autofill opt-in (default OFF). The disclosure spells out
              exactly what flows and that it never leaves the machine except into
              pages the user chooses. */}
          <div className="space-y-2 border-t border-foreground/10 pt-3">
            <Switch
              checked={autofillEnabled}
              onCheckedChange={(next) => void handleToggleAutofill(next)}
              disabled={setAutofill.isPending}
              label={t('settings.accounts.extension.autofill.label')}
              description={t('settings.accounts.extension.autofill.description')}
            />
            <p className="text-[11px] leading-snug text-foreground/40">
              {t('settings.accounts.extension.autofill.disclosure')}
            </p>
          </div>

          {/* Regenerate */}
          <div className="flex justify-end border-t border-foreground/10 pt-3">
            <Button
              variant="danger"
              disabled={regenerate.isPending}
              onClick={() => setConfirmOpen(true)}
            >
              {regenerate.isPending ? (
                <span className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />
              ) : (
                <RotateCcw size={11} />
              )}
              {t('settings.accounts.extension.regenerate')}
            </Button>
          </div>
        </div>
      </SettingsSection>

      <ConfirmModal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => void handleRegenerate()}
        title={t('settings.accounts.extension.regenerateConfirmTitle')}
        description={t('settings.accounts.extension.regenerateConfirmDescription')}
        confirmText={t('settings.accounts.extension.regenerateConfirm')}
        variant="danger"
        isConfirming={regenerate.isPending}
      />
    </>
  );
}
