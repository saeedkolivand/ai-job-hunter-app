import { type LucideIcon, Clock, FileText, Search, Sparkles, Briefcase } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import { useTranslation } from '@/lib/i18n';

interface ActivityItem {
  type: 'resume' | 'search' | 'ai' | 'document' | 'application';
  title: string;
  time: string;
  icon: LucideIcon;
}

const MOCK_ACTIVITIES: ActivityItem[] = [
  { type: 'resume', title: 'Analyzed resume.pdf', time: '2 hours ago', icon: FileText },
  { type: 'search', title: 'Searched "React Developer"', time: '4 hours ago', icon: Search },
  { type: 'ai', title: 'Generated cover letter', time: 'Yesterday', icon: Sparkles },
  { type: 'application', title: 'Applied to Senior Role', time: '2 days ago', icon: Briefcase },
];

const TYPE_COLORS: Record<ActivityItem['type'], string> = {
  resume: 'text-blue-400',
  search: 'text-green-400',
  ai: 'text-purple-400',
  document: 'text-orange-400',
  application: 'text-pink-400',
};

export function RecentActivity() {
  const { t } = useTranslation();

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          <Clock size={14} />
          {t('dashboard.recentActivity')}
        </div>
      </div>

      <div className="space-y-3">
        {MOCK_ACTIVITIES.map((activity) => {
          const Icon = activity.icon;
          return (
            <div
              key={activity.title}
              className="flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5 transition-colors hover:bg-white/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5">
                <Icon size={14} className={TYPE_COLORS[activity.type]} />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm text-foreground truncate">{activity.title}</div>
                <div className="text-xs text-foreground/40">{activity.time}</div>
              </div>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
