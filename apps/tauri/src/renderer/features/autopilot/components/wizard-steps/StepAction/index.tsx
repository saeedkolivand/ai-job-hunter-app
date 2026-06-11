import { Bell, ScanSearch, Wand2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

export function StepAction() {
  const { t } = useTranslation();

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
            className="flex items-start gap-3 rounded-xl border border-white/[0.05] bg-white/[0.02] px-4 py-3"
          >
            <Icon size={15} className="mt-0.5 shrink-0 text-brand-soft" />
            <div className="text-[11px] leading-relaxed text-foreground/70">{text}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
