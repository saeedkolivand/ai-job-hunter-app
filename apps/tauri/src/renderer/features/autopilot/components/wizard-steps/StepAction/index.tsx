import { BookOpen, Check, Send, ShieldAlert, Zap } from 'lucide-react';

import type { AutopilotAction } from '@ajh/shared';
import { Button, cn, TextArea } from '@ajh/ui';

import type { SetFn, WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';

import { WizardField } from '../WizardField';

interface StepActionProps {
  form: WizardState;
  set: SetFn;
}

export function StepAction({ form, set }: StepActionProps) {
  const { t } = useTranslation();
  const actionOptions = [
    {
      id: 'save' as AutopilotAction,
      label: t('autopilot.wizard.action.saveOnly'),
      desc: t('autopilot.wizard.action.saveOnlyDesc'),
      icon: BookOpen,
      color: 'text-blue-400',
    },
    {
      id: 'review' as AutopilotAction,
      label: t('autopilot.wizard.action.applyReview'),
      desc: t('autopilot.wizard.action.applyReviewDesc'),
      icon: Send,
      color: 'text-amber-400',
    },
    {
      id: 'auto_apply' as AutopilotAction,
      label: t('autopilot.wizard.action.autoApply'),
      desc: t('autopilot.wizard.action.autoApplyDesc'),
      icon: Zap,
      color: 'text-brand-soft',
    },
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
        {actionOptions.map(({ id, label, desc, icon: Icon, color }) => (
          <Button
            key={id}
            onClick={() => set('action', id)}
            className={cn(
              'w-full flex items-start gap-3 rounded-xl border px-4 py-3 text-left transition-all h-auto',
              form.action === id
                ? 'border-brand/35 bg-brand/08'
                : 'border-white/[0.05] hover:border-white/[0.08]'
            )}
          >
            <Icon size={15} className={cn('mt-0.5 shrink-0', color)} />
            <div className="flex-1 min-w-0">
              <div className="text-xs font-semibold text-foreground/80">{label}</div>
              <div className="text-[10px] text-foreground/40 mt-0.5">{desc}</div>
            </div>
            {form.action === id && <Check size={13} className="text-brand-soft shrink-0 mt-0.5" />}
          </Button>
        ))}
      </div>

      {form.action !== 'save' && (
        <WizardField
          label={t('autopilot.wizard.action.coverLetter')}
          hint={t('autopilot.wizard.action.coverLetterOptional')}
        >
          <TextArea
            value={form.coverLetter}
            onChange={(e) => set('coverLetter', e.target.value)}
            placeholder={t('autopilot.wizard.action.coverLetterPlaceholder')}
            className="w-full bg-white/[0.02] text-[11px] text-foreground/70 placeholder:text-foreground/20 font-mono leading-relaxed"
            rows={4}
            spellCheck={false}
          />
        </WizardField>
      )}

      {form.action === 'auto_apply' && (
        <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/[0.05] px-4 py-3">
          <ShieldAlert size={14} className="text-amber-400 shrink-0 mt-0.5" />
          <div className="text-[11px] text-amber-200/70">
            <span className="font-semibold text-amber-300/90">
              {t('autopilot.wizard.action.autoSubmitWarning')}
            </span>{' '}
            {t('autopilot.wizard.action.autoSubmitDesc')}
          </div>
          <Button
            onClick={() => set('autoSubmit', !form.autoSubmit)}
            className={cn(
              'shrink-0 rounded-full h-5 w-9 transition-colors relative p-0 border-transparent',
              form.autoSubmit ? 'bg-brand' : 'bg-white/10'
            )}
          >
            <span
              className={cn(
                'absolute top-0.5 h-4 w-4 rounded-full bg-white transition-transform',
                form.autoSubmit ? 'translate-x-4' : 'translate-x-0.5'
              )}
            />
          </Button>
        </div>
      )}
    </div>
  );
}
