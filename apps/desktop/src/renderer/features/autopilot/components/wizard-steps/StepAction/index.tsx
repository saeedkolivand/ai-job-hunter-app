import { Bell, ScanSearch, Wand2 } from 'lucide-react';
import { useFormContext, useWatch } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Switch } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';
import { useGenerateConfig } from '@/services';

export function StepAction() {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext<WizardState>();
  const assistant = useWatch({ control, name: 'assistant' });
  // The provider/model snapshot is GONE (task #16): a scheduled headless run now
  // resolves the CURRENTLY-active provider from the backend store at run time, so
  // there is nothing to capture here. `provider`/`model` are still read to gate
  // the toggle (no provider configured → disabled) and label the hint.
  const { provider, model, isPending } = useGenerateConfig();
  // Cold boot: while the backend config is first loading, `model` defaults to
  // '' — indistinguishable from a real "no provider configured" state. Show
  // nothing rather than falsely flashing the no-provider caption (mirrors
  // `useCanUseAI`'s isPending guard in ModelSelector).
  const caption = isPending
    ? null
    : model
      ? t('autopilot.wizard.action.assistantCaption')
      : t('autopilot.wizard.action.assistantNoProvider');

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
          disabled={!model}
        />
        {/* TODO(a11y): the shared `Switch` `description` slot renders at
            text-foreground/45 (~2.85:1 light) — a real AA fail (Switch.tsx:109).
            That's a separate systemic fix for the primitive; until then this
            feature's disclosure copy is its own properly-contrasted element
            instead of passing it through `description`, so the honesty copy
            this feature depends on stays legible. */}
        {caption && <p className="text-caption text-foreground/70">{caption}</p>}
        {assistant && provider && model && (
          <p className="text-caption text-foreground/70">
            {t('autopilot.wizard.action.assistantProviderHint', { provider, model })}
          </p>
        )}
      </div>
    </div>
  );
}
