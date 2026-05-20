import { type LucideIcon, Lightbulb, TrendingUp, AlertTriangle, CheckCircle } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';

interface Insight {
  type: 'suggestion' | 'alert' | 'match';
  title: string;
  description: string;
}

const INSIGHT_CONFIG: Record<Insight['type'], { icon: LucideIcon; color: string; bg: string }> = {
  suggestion: { icon: Lightbulb, color: 'text-yellow-400', bg: 'bg-yellow-500/20' },
  alert: { icon: AlertTriangle, color: 'text-orange-400', bg: 'bg-orange-500/20' },
  match: { icon: CheckCircle, color: 'text-green-400', bg: 'bg-green-500/20' },
};

export function AIInsights() {
  const { t } = useTranslation();
  const INSIGHTS: Insight[] = [
    {
      type: 'suggestion',
      title: t('dashboard.insights.addTsSuggestion'),
      description: t('dashboard.insights.addTsDesc'),
    },
    {
      type: 'match',
      title: t('dashboard.insights.highMatchTitle'),
      description: t('dashboard.insights.highMatchDesc'),
    },
    {
      type: 'alert',
      title: t('dashboard.insights.resumeAlertTitle'),
      description: t('dashboard.insights.resumeAlertDesc'),
    },
  ];

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <SectionLabel icon={TrendingUp}>{t('dashboard.aiInsights')}</SectionLabel>
      </div>

      <div className="space-y-2">
        {INSIGHTS.map((insight) => {
          const { icon: Icon, color, bg } = INSIGHT_CONFIG[insight.type];

          return (
            <div
              key={insight.title}
              className="flex items-start gap-3 rounded-lg bg-white/5 px-3 py-2.5 transition-all hover:bg-white/10"
            >
              <div className={cn('flex h-8 w-8 items-center justify-center rounded-full', bg)}>
                <Icon size={14} className={color} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm text-foreground">{insight.title}</div>
                <div className="text-xs text-foreground/40">{insight.description}</div>
              </div>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
