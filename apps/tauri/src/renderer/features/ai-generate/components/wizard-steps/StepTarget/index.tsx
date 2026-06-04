import { FileCheck, FileText, Sparkles } from 'lucide-react';

import { Button, cn } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

type GenTarget = 'resume' | 'cover' | 'both';

interface StepTargetProps {
  target: GenTarget;
  onTargetChange: (t: GenTarget) => void;
}

const TARGET_OPTIONS = [
  { id: 'resume' as const, icon: FileText, labelKey: 'aiGenerate.resume' },
  { id: 'cover' as const, icon: FileCheck, labelKey: 'aiGenerate.coverLetter' },
  { id: 'both' as const, icon: Sparkles, labelKey: 'aiGenerate.both' },
] as const;

export function StepTarget({ target, onTargetChange }: StepTargetProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">{t('aiGenerate.wizard.steps.0')}</p>
        <p className="text-xs text-foreground/35 mt-0.5">{t('aiGenerate.generate')}</p>
      </div>

      <div className="grid grid-cols-3 gap-3">
        {TARGET_OPTIONS.map(({ id, icon: Icon, labelKey }) => (
          <Button
            key={id}
            onClick={() => onTargetChange(id)}
            className={cn(
              'flex flex-col items-center gap-2 rounded-xl border py-6 text-xs font-medium transition-all h-auto',
              target === id
                ? 'border-brand/40 bg-brand/10 text-brand-soft'
                : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
            )}
          >
            <Icon size={20} />
            {t(labelKey)}
          </Button>
        ))}
      </div>
    </div>
  );
}
