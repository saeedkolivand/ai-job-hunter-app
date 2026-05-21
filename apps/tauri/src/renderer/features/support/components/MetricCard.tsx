interface MetricCardProps {
  label: string;
  value: string;
  trend: string;
  status?: 'healthy' | 'warning' | 'error';
}

export function MetricCard({ label, value, trend }: MetricCardProps) {
  const trendColor = trend.startsWith('+')
    ? 'text-emerald-400'
    : trend.startsWith('-')
      ? 'text-red-400'
      : 'text-foreground/40';

  return (
    <div className="glass-card rounded-xl p-4">
      <div className="mb-1 text-xs text-foreground/40">{label}</div>
      <div className="text-2xl font-semibold text-foreground">{value}</div>
      <div className={`mt-1 text-xs ${trendColor}`}>{trend}</div>
    </div>
  );
}
