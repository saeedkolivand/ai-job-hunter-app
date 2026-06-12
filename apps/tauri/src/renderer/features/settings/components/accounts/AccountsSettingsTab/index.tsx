import { Lock, ShieldAlert } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { SettingsSection } from '@ajh/ui';

import { AUTH_BOARDS } from '@/constants/auth';
import { useCredentialsAvailable } from '@/services';

import { BoardSessionRow } from '../BoardSessionRow';
import { ExtensionBridgeSection } from '../ExtensionBridgeSection';

export function AccountsSettingsTab() {
  const { t } = useTranslation();
  const { data: encAvail } = useCredentialsAvailable();

  return (
    <div className="space-y-3">
      {/* Subtle warning */}
      <div className="flex items-start gap-2.5 rounded-xl border border-amber-400/15 bg-amber-400/[0.06] px-4 py-3 text-xs text-amber-200/80">
        <ShieldAlert size={13} className="mt-0.5 shrink-0 text-amber-400/70" />
        <span>{t('settings.accounts.credentialsWarning')}</span>
      </div>

      {encAvail === false && (
        <div className="rounded-xl border border-red-400/15 bg-red-400/[0.06] px-4 py-3 text-xs text-red-200/80">
          {t('settings.accounts.noEncryptionWarning')}
        </div>
      )}

      <SettingsSection icon={Lock} label={t('settings.accounts.boardsTitle')}>
        <div className="space-y-3">
          {AUTH_BOARDS.map((board) => (
            <BoardSessionRow key={board.id} board={board} />
          ))}
        </div>
      </SettingsSection>

      <ExtensionBridgeSection />
    </div>
  );
}
