import { cn } from '@/lib/cn';

interface LogEntryProps {
  timestamp: string;
  level: 'info' | 'warning' | 'error';
  source: string;
  message: string;
}

export function LogEntry({ timestamp, level, source, message }: LogEntryProps) {
  const levelConfig = {
    info: { color: 'text-emerald-400', bg: 'bg-emerald-400/10' },
    warning: { color: 'text-amber-400', bg: 'bg-amber-400/10' },
    error: { color: 'text-red-400', bg: 'bg-red-400/10' },
  };

  const config = levelConfig[level];

  return (
    <div className="flex items-start gap-3 p-3 rounded-lg bg-white/[0.02] font-mono text-xs">
      <span className="text-foreground/40 shrink-0">{timestamp}</span>
      <span
        className={cn(
          'px-2 py-0.5 rounded shrink-0 uppercase tracking-wider',
          config.bg,
          config.color
        )}
      >
        {level}
      </span>
      <span className="text-foreground/40 shrink-0">[{source}]</span>
      <span className="text-foreground/90">{message}</span>
    </div>
  );
}
