import { CheckCircle2, XCircle } from 'lucide-react';

import { cn } from '@/lib/cn';

interface DiagnosticExportItemProps {
  name: string;
  included: boolean;
  description: string;
}

export function DiagnosticExportItem({ name, included, description }: DiagnosticExportItemProps) {
  return (
    <div className="flex items-start gap-3">
      <div className={cn('mt-0.5', included ? 'text-emerald-400' : 'text-foreground/40')}>
        {included ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
      </div>
      <div className="flex-1">
        <div className="text-sm font-medium text-foreground/90">{name}</div>
        <div className="text-xs text-foreground/55">{description}</div>
      </div>
    </div>
  );
}
