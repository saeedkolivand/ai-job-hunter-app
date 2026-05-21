import { ArrowRight, Clock, FileText, Play } from 'lucide-react';

import { Button, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface RecentProject {
  id: string;
  name: string;
  type: 'resume' | 'analysis' | 'document';
  lastModified: string;
}

const RECENT_PROJECTS: RecentProject[] = [
  {
    id: '1',
    name: 'Senior Frontend Developer - Google',
    type: 'analysis',
    lastModified: '2 hours ago',
  },
  { id: '2', name: 'My Resume 2024.pdf', type: 'resume', lastModified: 'Yesterday' },
  { id: '3', name: 'Job Market Analysis Q1', type: 'document', lastModified: '3 days ago' },
];

const TYPE_ICONS = {
  resume: FileText,
  analysis: Clock,
  document: FileText,
};

export function ContinueWorking() {
  const { t } = useTranslation();

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          <Play size={14} />
          {t('dashboard.continueWorking')}
        </div>
      </div>

      <div className="space-y-2">
        {RECENT_PROJECTS.map((project) => {
          const Icon = TYPE_ICONS[project.type];
          return (
            <div
              key={project.id}
              className="group flex items-center gap-3 rounded-lg bg-white/5 px-3 py-2.5 transition-all hover:bg-white/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full bg-white/5">
                <Icon size={14} className="text-foreground/40" />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm text-foreground truncate">{project.name}</div>
                <div className="text-xs text-foreground/40">{project.lastModified}</div>
              </div>
              <Button variant="ghost" size="sm" className="!bg-transparent hover:bg-white/5">
                <ArrowRight size={14} />
              </Button>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
