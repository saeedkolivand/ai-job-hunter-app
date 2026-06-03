import { ArrowLeft, ArrowRight, Check, Eye, EyeOff, Key, Loader2, Search } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { Button, FloatingIcon, Input, useNotification, withDelay } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useHasProviderKey, useOpenExternal, useSetProviderKey } from '@/services';

import { OnboardingStepWrapper } from '../../components/OnboardingStepWrapper';

const KEYS_URL = 'https://ollama.com/settings/keys';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

/**
 * Optional, skippable onboarding step: connect the free Ollama account key so
 * company research works on Ollama. Cloud/CLI providers already search with
 * their own key, so this step is never required to advance.
 */
export function ResearchStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const setProviderKey = useSetProviderKey();
  const { data: keyData } = useHasProviderKey('ollama-cloud');
  const connected = keyData?.has ?? false;

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      await setProviderKey.mutateAsync({ provider: 'ollama-cloud', apiKey: apiKey.trim() });
      setApiKey('');
      notify(t('onboarding.research.saved'), 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : t('onboarding.research.saveError'), 'error');
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
          {t('onboarding.research.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.research.subtitle')}</p>
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.15)}
        className="mb-6"
      >
        {connected ? (
          <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5 text-sm text-emerald-300/80">
            <Key size={13} className="text-emerald-400" />
            {t('onboarding.research.connected')}
          </div>
        ) : (
          <div className="space-y-2">
            <p className="text-xs text-foreground/40">
              {t('onboarding.research.getKeyAt')}{' '}
              <Button
                variant="unstyled"
                onClick={() => void openExternal.mutateAsync(KEYS_URL)}
                className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
              >
                {KEYS_URL.replace('https://', '')}
              </Button>
            </p>
            <div className="relative">
              <Input
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void handleSave()}
                placeholder={t('onboarding.research.keyPlaceholder')}
                className="w-full pr-9 text-sm"
              />
              <Button
                variant="unstyled"
                onClick={() => setShowKey((v) => !v)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </Button>
            </div>
            <div className="flex justify-end">
              <Button
                variant="glass"
                size="sm"
                disabled={!apiKey.trim() || saving}
                onClick={() => void handleSave()}
                className={apiKey.trim() && !saving ? 'ring-1 ring-brand/20' : ''}
              >
                {saving ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <>
                    <Check size={12} /> {t('onboarding.research.saveKey')}
                  </>
                )}
              </Button>
            </div>
            <p className="text-[10px] text-foreground/30">
              {t('onboarding.research.optionalNote')}
            </p>
          </div>
        )}
      </motion.div>

      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={withDelay(0.2)}
        className="flex items-center gap-3"
      >
        <Button variant="ghost" size="sm" onClick={onBack} className="flex items-center gap-1.5">
          <ArrowLeft size={13} />
          {t('onboarding.research.back')}
        </Button>

        <div className="flex-1" />

        <Button variant="default" size="sm" onClick={onNext} className="flex items-center gap-1.5">
          {connected ? t('onboarding.research.next') : t('onboarding.research.skip')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
