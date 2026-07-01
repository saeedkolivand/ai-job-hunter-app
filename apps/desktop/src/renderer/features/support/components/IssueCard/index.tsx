import { AlertTriangle } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

interface IssueCardProps {
  title: string;
  solutions: string[];
}

export function IssueCard({ title, solutions }: IssueCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation();

  return (
    <div className="glass-card rounded-xl p-4">
      <Button
        onClick={() => setExpanded(!expanded)}
        className="flex items-start gap-3 w-full text-left h-auto bg-transparent border-transparent"
      >
        <AlertTriangle size={16} className="mt-0.5 text-amber-400 shrink-0" />
        <div className="flex-1">
          <div className="text-sm font-medium text-foreground/90">{title}</div>
        </div>
      </Button>
      {expanded && (
        <div className="mt-3 ml-7 space-y-2">
          <div className="text-xs font-medium text-foreground/40 mb-2">
            {t('support.aiRuntime.suggestedFixes')}
          </div>
          {solutions.map((solution, i) => (
            <div key={i} className="flex items-start gap-2 text-xs text-foreground/70">
              <span className="text-brand-soft mt-1">•</span>
              <span>{solution}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
