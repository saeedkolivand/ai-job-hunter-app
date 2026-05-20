import {
  type LucideIcon,
  Cpu,
  Database,
  Activity,
  CheckCircle,
  XCircle,
  Loader2,
} from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';

interface SystemStatus {
  name: string;
  status: 'ready' | 'loading' | 'error';
  icon: LucideIcon;
  details?: string;
}

const SYSTEM_STATUS: SystemStatus[] = [
  { name: 'AI Model', status: 'ready', icon: Cpu, details: 'llama3:8b' },
  { name: 'Ollama', status: 'ready', icon: Cpu, details: 'Running' },
  { name: 'Vector DB', status: 'ready', icon: Database, details: 'Indexed' },
  { name: 'Workers', status: 'ready', icon: Activity, details: '2 active' },
];

const STATUS_CONFIG = {
  ready: { icon: CheckCircle, color: 'text-green-400', bg: 'bg-green-500/20' },
  loading: { icon: Loader2, color: 'text-yellow-400', bg: 'bg-yellow-500/20' },
  error: { icon: XCircle, color: 'text-red-400', bg: 'bg-red-500/20' },
};

export function AISystemStatus() {
  const { t } = useTranslation();
  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          <Cpu size={14} />
          {t('dashboard.aiSystemStatus')}
        </div>
      </div>

      <div className="space-y-2">
        {SYSTEM_STATUS.map((system) => {
          const config = STATUS_CONFIG[system.status];
          const StatusIcon = config.icon;
          const SystemIcon = system.icon;

          return (
            <div
              key={system.name}
              className="flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5">
                <SystemIcon size={14} className="text-foreground/40" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm text-foreground">{system.name}</div>
                <div className="text-xs text-foreground/40">{system.details}</div>
              </div>
              <div
                className={cn('flex h-6 w-6 items-center justify-center rounded-full', config.bg)}
              >
                <StatusIcon
                  size={14}
                  className={cn(system.status === 'loading' && 'animate-spin', config.color)}
                />
              </div>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
