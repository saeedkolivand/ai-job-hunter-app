import { cn } from '@ajh/ui';

interface Props {
  label: string;
  ready: boolean;
  detail?: string;
}

export function StatusDot({ label, ready, detail }: Props) {
  return (
    <span className="flex items-center gap-1.5">
      <span
        className={cn('h-1.5 w-1.5 rounded-full', ready ? 'bg-emerald-400' : 'bg-amber-400/70')}
      />
      <span className={cn(ready ? 'text-foreground/55' : 'text-foreground/35')}>
        {label}
        {detail ? ` · ${detail}` : ''}
      </span>
    </span>
  );
}
