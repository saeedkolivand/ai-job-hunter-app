import { Languages, User, Wand2 } from 'lucide-react';

import { Button, GlassCard, IconBadge, Input, SectionLabel } from '@ajh/ui';

import { LanguageSelector } from '@/features/settings/components/shared/LanguageSelector';
import { UpdateSection } from '@/features/settings/components/update-section';
import { useTranslation } from '@/lib/i18n';
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

      <UpdateSection />
    </>
  );
}
