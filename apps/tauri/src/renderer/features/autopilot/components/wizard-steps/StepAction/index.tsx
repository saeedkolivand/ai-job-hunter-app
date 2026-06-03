import { Bell, FileText, ScanSearch, Wand2 } from 'lucide-react';

import { TextArea } from '@ajh/ui';

import type { SetFn, WizardState } from '@/features/autopilot/types';
import { useTranslation } from '@/lib/i18n';

import { WizardField } from '../WizardField';

interface StepActionProps {
  form: WizardState;
  set: SetFn;
}

export function StepAction({ form, set }: StepActionProps) {
  const { t } = useTranslation();

  // Autopilot is a discovery assistant: it finds & ranks matching jobs and
  // notifies you, then you apply with the tailoring assistant. It never submits
  // applications on your behalf — so this step is informational plus an optional
  // base cover letter the assistant reuses when tailoring each found job.
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
            className="flex items-start gap-3 rounded-xl border border-white/[0.05] bg-white/[0.02] px-4 py-3"
          >
            <Icon size={15} className="mt-0.5 shrink-0 text-brand-soft" />
            <div className="text-[11px] leading-relaxed text-foreground/70">{text}</div>
          </div>
        ))}
      </div>

      <WizardField
        label={t('autopilot.wizard.action.coverLetter')}
        hint={t('autopilot.wizard.action.coverLetterOptional')}
      >
        <div className="mb-2 flex items-start gap-2 text-[10px] leading-relaxed text-foreground/40">
          <FileText size={11} className="mt-0.5 shrink-0" />
          <span>{t('autopilot.wizard.action.coverLetterHint')}</span>
        </div>
        <TextArea
          value={form.coverLetter}
          onChange={(e) => set('coverLetter', e.target.value)}
          placeholder={t('autopilot.wizard.action.coverLetterPlaceholder')}
          className="w-full bg-white/[0.02] text-[11px] text-foreground/70 placeholder:text-foreground/20 font-mono leading-relaxed"
          rows={5}
          spellCheck={false}
        />
      </WizardField>
    </div>
  );
}
