import { Lock } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Alert, SettingsSection } from '@ajh/ui';

import { AUTH_BOARDS } from '@/constants/auth';
import { useCredentialsAvailable } from '@/services';

import { BoardSessionRow } from '../BoardSessionRow';
import { ExtensionBridgeSection } from '../ExtensionBridgeSection';

export function AccountsSettingsTab() {
  const { t } = useTranslation();
  const { data: encAvail } = useCredentialsAvailable();

  return (
    <div className="space-y-3">
      <Alert type="warning" showIcon message={t('settings.accounts.credentialsWarning')} />

      {encAvail === false && (
        <Alert type="error" showIcon message={t('settings.accounts.noEncryptionWarning')} />
      )}

      <div data-settings-anchor="accounts-boards">
        <SettingsSection icon={Lock} label={t('settings.accounts.boardsTitle')}>
          <div className="space-y-3">
            {AUTH_BOARDS.map((board) => (
              <BoardSessionRow key={board.id} board={board} />
            ))}
          </div>
        </SettingsSection>
      </div>

      <div data-settings-anchor="accounts-extension">
        <ExtensionBridgeSection />
      </div>
    </div>
  );
}
