import { Bell, ScanSearch, Wand2 } from 'lucide-react';
import { useEffect } from 'react';
import { useFormContext, useWatch } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Switch } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';
import { useGenerateConfig } from '@/services';

export function StepAction() {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext<WizardState>();
  const assistant = useWatch({ control, name: 'assistant' });
  const assistantProvider = useWatch({ control, name: 'assistantProvider' });
  const assistantModel = useWatch({ control, name: 'assistantModel' });
  const assistantBaseUrl = useWatch({ control, name: 'assistantBaseUrl' });
  const { provider, model, baseUrl } = useGenerateConfig();

  // The scheduled run is headless (no renderer), so it can't resolve "the
  // active provider" itself — it replays whatever was snapshotted here. Keep
  // the snapshot in sync with the live provider whenever notes are on: this
  // covers both "just enabled" (snapshot starts empty) and re-opening an edit
  // whose snapshot has since gone stale (the user switched provider in
  // Settings after this autopilot was last saved). The equality guard makes
  // this a no-op once in sync, so it never loops or dirties the form for no
  // reason.
  useEffect(() => {
    if (!assistant) return;
    if (
      provider === assistantProvider &&
      model === assistantModel &&
      baseUrl === assistantBaseUrl
    ) {
      return;
    }
    setValue('assistantProvider', provider, { shouldDirty: true });
    setValue('assistantModel', model, { shouldDirty: true });
    setValue('assistantBaseUrl', baseUrl, { shouldDirty: true });
  }, [
    assistant,
    provider,
    model,
    baseUrl,
    assistantProvider,
    assistantModel,
    assistantBaseUrl,
    setValue,
  ]);

  // Autopilot is a discovery assistant: it finds & ranks matching jobs and
  // notifies you, then you apply with the tailoring assistant on the dedicated
  // apply page. It never submits applications on your behalf — so this step is
  // purely informational.
  const flow = [
    { icon: ScanSearch, text: t('autopilot.wizard.action.flowFind') },
    { icon: Bell, text: t('autopilot.wizard.action.flowNotify') },
    { icon: Wand2, text: t('autopilot.wizard.action.flowTailor') },
  ];

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.wizard.action.title')}
        </p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('autopilot.wizard.action.subtitle')}</p>
      </div>

      <div className="space-y-2">
        {flow.map(({ icon: Icon, text }, i) => (
          <div
            key={i}
            className="flex items-start gap-3 rounded-xl border border-[var(--border-clear)] bg-card px-4 py-3"
          >
            <Icon size={15} className="mt-0.5 shrink-0 text-brand-soft" />
            <div className="text-[11px] leading-relaxed text-foreground/70">{text}</div>
          </div>
        ))}
      </div>

      <div className="rounded-xl border border-[var(--border-clear)] bg-card px-4 py-3 space-y-2">
        <Switch
          label={t('autopilot.wizard.action.assistantLabel')}
          checked={assistant}
          onCheckedChange={(next) => setValue('assistant', next, { shouldDirty: true })}
        />
        {/* TODO(a11y): the shared `Switch` `description` slot renders at
            text-foreground/45 (~2.85:1 light) — a real AA fail (Switch.tsx:109).
            That's a separate systemic fix for the primitive; until then this
            feature's disclosure copy is its own properly-contrasted element
            instead of passing it through `description`, so the honesty copy
            this feature depends on stays legible. */}
        <p className="text-caption text-foreground/70">
          {t('autopilot.wizard.action.assistantCaption')}
        </p>
        {assistant && (
          <p className="text-caption text-foreground/70">
            {t('autopilot.wizard.action.assistantProviderHint', { provider, model })}
          </p>
        )}
      </div>
    </div>
  );
}
