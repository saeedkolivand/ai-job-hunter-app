import { Languages, Power, User, Wand2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, Input, SettingsSection, Switch } from '@ajh/ui';

import { LanguageSelector } from '@/features/settings/components/shared/LanguageSelector';
import { UpdateSection } from '@/features/settings/components/update-section';
import { useLaunchAtLogin, useSetCloseToTray, useSetLaunchAtLogin } from '@/services';
import {
  useCloseToTray,
  useOnboardingCompleted,
  usePreferencesStore,
} from '@/store/preferences-store';

interface GeneralSectionProps {
  localName: string;
  setLocalName: (v: string) => void;
  setUserName: (v: string) => void;
  userName: string | undefined;
}

export function GeneralSection({
  localName,
  setLocalName,
  setUserName,
  userName,
}: GeneralSectionProps) {
  const { t } = useTranslation();
  const onboardingCompleted = useOnboardingCompleted();
  const resetOnboarding = usePreferencesStore((s) => s.resetOnboarding);
  const replayWizard = () => resetOnboarding();

  const { data: launchAtLogin = false } = useLaunchAtLogin();
  const setLaunchAtLogin = useSetLaunchAtLogin();

  const closeToTray = useCloseToTray();
  const setCloseToTray = useSetCloseToTray();

  return (
    <>
      <SettingsSection icon={User} label={t('settings.profile.title')}>
        <div className="space-y-3">
          <label className="block">
            <div className="mb-1.5 text-[11px] font-medium text-foreground/50">
              {t('settings.profile.displayName')}
            </div>
            <div className="flex gap-2">
              <Input
                value={localName}
                onChange={(e) => setLocalName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && localName !== (userName || '')) setUserName(localName);
                }}
                placeholder={t('settings.profile.placeholder')}
                className="flex-1"
              />
              <Button
                variant="glass"
                onClick={() => setUserName(localName)}
                disabled={localName === (userName || '')}
              >
                {t('settings.profile.save')}
              </Button>
            </div>
          </label>
        </div>
      </SettingsSection>

      <SettingsSection icon={Languages} label={t('settings.language.title')}>
        <LanguageSelector />
      </SettingsSection>

      <SettingsSection icon={Wand2} label={t('settings.onboarding.title')}>
        <div className="flex items-center justify-between">
          <p className="text-xs text-foreground/45">{t('settings.onboarding.description')}</p>
          <Button
            variant="glass"
            onClick={replayWizard}
            disabled={!onboardingCompleted}
            className="ml-4 shrink-0"
          >
            {t('settings.onboarding.replay')}
          </Button>
        </div>
      </SettingsSection>

      <SettingsSection icon={Power} label={t('settings.startup.title')}>
        <div className="space-y-2.5">
          <div className="rounded-lg border border-foreground/10 px-3 py-2.5">
            <Switch
              label={t('settings.startup.launchAtLogin')}
              description={t('settings.startup.launchAtLoginHint')}
              checked={launchAtLogin}
              disabled={setLaunchAtLogin.isPending}
              onCheckedChange={(next) => setLaunchAtLogin.mutate(next)}
            />
          </div>
          <div className="rounded-lg border border-foreground/10 px-3 py-2.5">
            <Switch
              label={t('settings.startup.closeToTray')}
              description={t('settings.startup.closeToTrayHint')}
              checked={closeToTray}
              disabled={setCloseToTray.isPending}
              onCheckedChange={(next) => setCloseToTray.mutate(next)}
            />
          </div>
        </div>
      </SettingsSection>

      <UpdateSection />
    </>
  );
}
