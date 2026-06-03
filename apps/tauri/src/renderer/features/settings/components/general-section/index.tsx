import { Languages, Power, User, Wand2 } from 'lucide-react';

import { Button, cn, GlassCard, IconBadge, Input, SectionLabel } from '@ajh/ui';

import { AppearanceCard } from '@/features/settings/components/general-section/AppearanceCard';
import { LanguageSelector } from '@/features/settings/components/shared/LanguageSelector';
import { UpdateSection } from '@/features/settings/components/update-section';
import { useTranslation } from '@/lib/i18n';
import { useLaunchAtLogin, useSetLaunchAtLogin } from '@/services';
import { useOnboardingCompleted, usePreferencesStore } from '@/store/preferences-store';

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
  const replayWizard = () =>
    usePreferencesStore.setState((s) => ({ ...s, onboardingCompleted: false }));

  const { data: launchAtLogin = false } = useLaunchAtLogin();
  const setLaunchAtLogin = useSetLaunchAtLogin();

  return (
    <>
      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={User} size="sm" />
          <SectionLabel>{t('settings.profile.title')}</SectionLabel>
        </div>
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
                size="sm"
                onClick={() => setUserName(localName)}
                disabled={localName === (userName || '')}
              >
                {t('settings.profile.save')}
              </Button>
            </div>
          </label>
        </div>
      </GlassCard>

      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={Languages} size="sm" />
          <SectionLabel>{t('settings.language.title')}</SectionLabel>
        </div>
        <LanguageSelector />
      </GlassCard>

      <AppearanceCard />

      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={Wand2} size="sm" />
          <SectionLabel>{t('settings.onboarding.title')}</SectionLabel>
        </div>
        <div className="flex items-center justify-between">
          <p className="text-xs text-foreground/45">{t('settings.onboarding.description')}</p>
          <Button
            variant="glass"
            size="sm"
            onClick={replayWizard}
            disabled={!onboardingCompleted}
            className="ml-4 shrink-0"
          >
            {t('settings.onboarding.replay')}
          </Button>
        </div>
      </GlassCard>

      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={Power} size="sm" />
          <SectionLabel>{t('settings.startup.title')}</SectionLabel>
        </div>
        <Button
          variant="unstyled"
          type="button"
          role="switch"
          aria-checked={launchAtLogin}
          disabled={setLaunchAtLogin.isPending}
          onClick={() => setLaunchAtLogin.mutate(!launchAtLogin)}
          className={cn(
            'flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-left transition-all disabled:opacity-50',
            launchAtLogin
              ? 'border-brand/35 bg-brand/10'
              : 'border-white/[0.05] bg-transparent hover:border-white/[0.08]'
          )}
        >
          <div>
            <div
              className={cn(
                'text-[11px] font-medium',
                launchAtLogin ? 'text-foreground/90' : 'text-foreground/55'
              )}
            >
              {t('settings.startup.launchAtLogin')}
            </div>
            <div className="mt-0.5 text-[10px] text-foreground/35">
              {t('settings.startup.launchAtLoginHint')}
            </div>
          </div>
          <div
            className={cn(
              'relative ml-3 h-4 w-7 shrink-0 rounded-full transition-colors',
              launchAtLogin ? 'bg-brand' : 'bg-white/10'
            )}
          >
            <div
              className={cn(
                'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                launchAtLogin ? 'translate-x-3.5' : 'translate-x-0.5'
              )}
            />
          </div>
        </Button>
      </GlassCard>

      <UpdateSection />
    </>
  );
}
