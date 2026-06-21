import { ArrowLeft, ArrowRight, Check, Eye, EyeOff, Key, Loader2, Search } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, FloatingIcon, Input, useNotification, withDelay } from '@ajh/ui';

import { useHasProviderKey, useOpenExternal, useSetProviderKey } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

const ADZUNA_DOCS_URL = 'https://developer.adzuna.com';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Optional, skippable onboarding step: connect a free Adzuna API key so the
 * aggregator board returns results. Both App ID and App Key are needed; the
 * user can skip and add them later via Settings → Jobs.
 */
export function AdzunaKeyStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();

  const { data: idData } = useHasProviderKey('adzuna-app-id');
  const { data: keyData } = useHasProviderKey('adzuna-app-key');
  const idSaved = idData?.has ?? false;
  const keySaved = keyData?.has ?? false;
  const bothSaved = idSaved && keySaved;

  const [appId, setAppId] = useState('');
  const [appKey, setAppKey] = useState('');
  const [showId, setShowId] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  const hasSomethingToSave = appId.trim().length > 0 || appKey.trim().length > 0;

  const handleSave = async () => {
    if (!appId.trim() && !appKey.trim()) return;
    setSaving(true);
    try {
      if (appId.trim()) {
        await setProviderKey.mutateAsync({ provider: 'adzuna-app-id', apiKey: appId.trim() });
        setAppId('');
      }
      if (appKey.trim()) {
        await setProviderKey.mutateAsync({ provider: 'adzuna-app-key', apiKey: appKey.trim() });
        setAppKey('');
      }
      notify.success({ message: t('onboarding.adzunaKey.saved') });
    } catch (err) {
      notify.error({
        message: err instanceof Error ? err.message : t('onboarding.adzunaKey.saveError'),
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance
    >
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={Search} size={24} />
      </div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.1)}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.adzunaKey.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.adzunaKey.subtitle')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-6"
      >
        {bothSaved ? (
          <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5 text-sm text-emerald-300/80">
            <Key size={13} className="text-emerald-400" />
            {t('onboarding.adzunaKey.connected')}
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-xs text-foreground/40">
              {t('onboarding.adzunaKey.getKeyAt')}{' '}
              <Button
                variant="unstyled"
                onClick={() => void openExternal.mutateAsync(ADZUNA_DOCS_URL)}
                className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
              >
                developer.adzuna.com
              </Button>
            </p>

            {/* App ID field */}
            <div>
              {idSaved ? (
                <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2 text-xs text-emerald-300/80">
                  <Key size={12} className="text-emerald-400" />
                  {t('onboarding.adzunaKey.appIdLabel')} —{' '}
                  {t('settings.aggregatorKeys.adzunaAppId.connected')}
                </div>
              ) : (
                <div className="relative">
                  <label
                    htmlFor="onboarding-adzuna-app-id"
                    className="mb-1 block text-xs font-medium text-foreground/55"
                  >
                    {t('onboarding.adzunaKey.appIdLabel')}
                  </label>
                  <div className="relative">
                    <Input
                      id="onboarding-adzuna-app-id"
                      type={showId ? 'text' : 'password'}
                      value={appId}
                      onChange={(e) => setAppId(e.target.value)}
                      onKeyDown={(e) =>
                        e.key === 'Enter' && hasSomethingToSave && !saving && void handleSave()
                      }
                      placeholder={t('onboarding.adzunaKey.appIdPlaceholder')}
                      className="w-full pr-9 text-sm"
                    />
                    <Button
                      variant="unstyled"
                      aria-label={t(
                        showId ? 'settings.aiProvider.hideKey' : 'settings.aiProvider.showKey'
                      )}
                      onClick={() => setShowId((v) => !v)}
                      className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
                    >
                      {showId ? <EyeOff size={13} /> : <Eye size={13} />}
                    </Button>
                  </div>
                </div>
              )}
            </div>

            {/* App Key field */}
            <div>
              {keySaved ? (
                <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2 text-xs text-emerald-300/80">
                  <Key size={12} className="text-emerald-400" />
                  {t('onboarding.adzunaKey.appKeyLabel')} —{' '}
                  {t('settings.aggregatorKeys.adzunaAppKey.connected')}
                </div>
              ) : (
                <div>
                  <label
                    htmlFor="onboarding-adzuna-app-key"
                    className="mb-1 block text-xs font-medium text-foreground/55"
                  >
                    {t('onboarding.adzunaKey.appKeyLabel')}
                  </label>
                  <div className="relative">
                    <Input
                      id="onboarding-adzuna-app-key"
                      type={showKey ? 'text' : 'password'}
                      value={appKey}
                      onChange={(e) => setAppKey(e.target.value)}
                      onKeyDown={(e) =>
                        e.key === 'Enter' && hasSomethingToSave && !saving && void handleSave()
                      }
                      placeholder={t('onboarding.adzunaKey.appKeyPlaceholder')}
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
                      {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
                    </Button>
                  </div>
                </div>
              )}
            </div>

            {(idSaved || keySaved) && !bothSaved && (
              <p className="text-xs text-amber-400/70">{t('onboarding.adzunaKey.partialSaved')}</p>
            )}

            <div className="flex justify-end">
              <Button
                variant="primary"
                disabled={!hasSomethingToSave || saving}
                onClick={() => void handleSave()}
              >
                {saving ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <>
                    <Check size={12} /> {t('onboarding.adzunaKey.save')}
                  </>
                )}
              </Button>
            </div>

            <p className="text-[10px] text-foreground/30">{t('onboarding.adzunaKey.skipHint')}</p>
          </div>
        )}
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        <Button variant="ghost" onClick={onBack} className="flex items-center gap-1.5">
          <ArrowLeft size={13} />
          {t('onboarding.adzunaKey.back')}
        </Button>

        <div className="flex-1" />

        <Button variant="primary" onClick={onNext} className="flex items-center gap-1.5">
          {bothSaved ? t('onboarding.adzunaKey.next') : t('onboarding.adzunaKey.skip')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
