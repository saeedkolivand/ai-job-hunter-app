import { FileCheck, FileText, Sparkles } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

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
    <div className="grid grid-cols-1 gap-3 @xs:grid-cols-3">
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
  );
}
