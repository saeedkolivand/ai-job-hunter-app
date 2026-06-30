import { cn } from '@ajh/ui';

interface Props {
  label: string;
  value: string | number;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  color: string;
  bg: string;
  animate?: boolean;
}

export function MetricCard({ label, value, icon: Icon, color, bg, animate }: Props) {
  return (
    <div className="relative overflow-hidden rounded-xl border border-foreground/[0.08] bg-foreground/[0.03] px-4 py-4">
      <div className={cn('mb-3 flex h-8 w-8 items-center justify-center rounded-lg', bg)}>
        <Icon size={15} className={cn(color, animate && 'animate-spin')} />
      </div>
      <div className="text-3xl font-bold tabular-nums text-foreground">{value}</div>
      <div className="mt-0.5 text-[11px] text-foreground/40">{label}</div>
    </div>
  );
}
