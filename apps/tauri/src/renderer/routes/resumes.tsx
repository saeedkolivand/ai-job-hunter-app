import { Bookmark, Eye, RefreshCw, Search, Send, Wand2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { Button, CardSkeleton, EmptyState, Input } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { GenerationCard } from '@/features/resumes/components/GenerationCard';
import { InteractionRow } from '@/features/resumes/components/InteractionRow';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { stagger, transition } from '@/lib/motion';
import { useAiGenerations } from '@/services/use-ai-generations';
import { useInteractions } from '@/services/use-postings';
import { useSessionStore } from '@/store/session-store';

export const Route = createFileRoute('/resumes')({ component: Resumes });

interface Interaction {
  jobId: string;
  interactionType: string;
  timestamp: number;
  title: string;
  company: string;
  url: string;
  source: string;
  location: string;
}

type Tab = 'applied' | 'viewed' | 'bookmarked' | 'generated';

const TAB_CONFIG = [
  {
    id: 'applied' as Tab,
    labelKey: 'resumes.tabs.applied',
    icon: Send,
    color: 'text-purple-300',
    ringColor: 'border-purple-400/30 bg-purple-400/10',
  },
  {
    id: 'viewed' as Tab,
    labelKey: 'resumes.tabs.viewed',
    icon: Eye,
    color: 'text-blue-300',
    ringColor: 'border-blue-400/30 bg-blue-400/10',
  },
  {
    id: 'bookmarked' as Tab,
    labelKey: 'resumes.tabs.bookmarked',
    icon: Bookmark,
    color: 'text-amber-300',
    ringColor: 'border-amber-400/30 bg-amber-400/10',
  },
  {
    id: 'generated' as Tab,
    labelKey: 'resumes.tabs.generated',
    icon: Wand2,
    color: 'text-brand-soft',
    ringColor: 'border-brand/30 bg-brand/10',
  },
] as const;

function Resumes() {
  const { t } = useTranslation();
  const { resumes, setResumes } = useSessionStore();
  const { tab, filter } = resumes;
  const setTab = (v: Tab) => setResumes({ tab: v });
  const setFilter = (v: string) => setResumes({ filter: v });

  const isGeneratedTab = tab === 'generated';

  const { data: rows = [], isLoading, refetch } = useInteractions(isGeneratedTab ? 'applied' : tab);

  const { data: generations = [] } = useAiGenerations();

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q
      ? (rows as Interaction[]).filter(
          (r) => r.title.toLowerCase().includes(q) || r.company.toLowerCase().includes(q)
        )
      : (rows as Interaction[]);
  }, [rows, filter]);

  const filteredGenerations = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q
      ? (generations as AiGenerationRecord[]).filter(
          (g) =>
            g.jobTitle.toLowerCase().includes(q) ||
            g.companyName.toLowerCase().includes(q) ||
            g.candidateName.toLowerCase().includes(q)
        )
      : (generations as AiGenerationRecord[]);
  }, [generations, filter]);

  const tabCfg = TAB_CONFIG.find((c) => c.id === tab) as (typeof TAB_CONFIG)[number];

  const tabCount = isGeneratedTab ? generations.length : rows.length;

  return (
    <PageTransition className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-10 py-10">
        <PageHeader
          title={t('resumes.title')}
          subtitle={t('resumes.subtitle')}
          badge={t('resumes.badge')}
          actions={
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-1.5 transition-colors focus-within:border-brand/35">
                <Search size={12} className="shrink-0 text-foreground/40" />
                <Input
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder={t('resumes.filterPlaceholder')}
                  className="w-40 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25 border-none p-0 rounded-none"
                  variant="default"
                />
              </div>
              {!isGeneratedTab && (
                <Button size="sm" variant="ghost" onClick={() => void refetch()} title="Refresh">
                  <RefreshCw size={12} className={isLoading ? 'animate-spin' : ''} />
                </Button>
              )}
            </div>
          }
        />

        {/* Tabs */}
        <div className="mb-5 flex items-center gap-1">
          {TAB_CONFIG.map(({ id, labelKey, icon: Icon, color }) => (
            <Button
              key={id}
              onClick={() => {
                setTab(id);
                setFilter('');
              }}
              className={cn(
                'flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-150 h-auto',
                tab === id
                  ? 'bg-white/[0.07] text-foreground/90 ring-1 ring-white/10'
                  : 'text-foreground/45 hover:bg-white/[0.04] hover:text-foreground/70'
              )}
            >
              <Icon size={12} className={tab === id ? color : ''} />
              {t(labelKey)}
              {tab === id && tabCount > 0 && (
                <span className="rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] text-foreground/60">
                  {tabCount}
                </span>
              )}
            </Button>
          ))}
        </div>

        {/* Generated tab content */}
        {isGeneratedTab ? (
          filteredGenerations.length === 0 ? (
            <EmptyState
              icon={Wand2}
              title={t('resumes.generated.noGenerationsYet')}
              description={t('resumes.generated.noGenerationsDesc')}
            />
          ) : (
            <motion.div
              className="flex flex-col gap-3"
              variants={stagger.container}
              initial="hidden"
              animate="show"
            >
              <AnimatePresence initial={false}>
                {filteredGenerations.map((gen) => (
                  <motion.div
                    key={gen.id}
                    variants={stagger.item}
                    transition={transition.normal}
                    exit={{ opacity: 0, y: -6 }}
                  >
                    <GenerationCard gen={gen} />
                  </motion.div>
                ))}
              </AnimatePresence>
            </motion.div>
          )
        ) : isLoading ? (
          <div className="space-y-2">
            <CardSkeleton /> <CardSkeleton /> <CardSkeleton />
          </div>
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={tabCfg.icon}
            title={
              filter ? t('resumes.noResults') : t('resumes.noJobsYet', { tab: t(tabCfg.labelKey) })
            }
            description={!filter ? t('resumes.jobsWillAppear') : undefined}
          />
        ) : (
          <motion.div
            className="flex flex-col gap-2"
            variants={stagger.container}
            initial="hidden"
            animate="show"
          >
            <AnimatePresence initial={false}>
              {filtered.map((row) => (
                <motion.div
                  key={`${row.jobId}-${row.interactionType}`}
                  variants={stagger.item}
                  transition={transition.normal}
                  exit={{ opacity: 0, y: -6 }}
                >
                  <InteractionRow row={row} tabCfg={tabCfg} />
                </motion.div>
              ))}
            </AnimatePresence>
          </motion.div>
        )}
      </div>
    </PageTransition>
  );
}
