import { Activity, Mail, RefreshCw, Search, Trash2, Wand2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useState } from 'react';

import {
  Button,
  CardSkeleton,
  cn,
  ConfirmModal,
  EmptyState,
  Input,
  stagger,
  transition,
} from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { GenerationCard } from '@/features/resumes/components/GenerationCard';
import { InteractionRow } from '@/features/resumes/components/InteractionRow';
import { DOC_TABS, type DocTab, type Interaction } from '@/features/resumes/constants';
import { useTranslation } from '@/lib/i18n';
import { useAiGenerations, useRemoveAiGenerationsBulk } from '@/services/use-ai-generations';
import { useInteractions } from '@/services/use-postings';
import { useSessionStore } from '@/store/session-store';

function ResumesPage() {
  const { t } = useTranslation();
  const { resumes, setResumes } = useSessionStore();
  const { tab, filter } = resumes;
  const setTab = (v: DocTab) => {
    setResumes({ tab: v });
    setSelection(new Set());
  };
  const setFilter = (v: string) => setResumes({ filter: v });

  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [confirmBulkDelete, setConfirmBulkDelete] = useState(false);
  const removeBulk = useRemoveAiGenerationsBulk();

  const toggleSelect = (id: string) => {
    setSelection((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = (ids: string[]) => {
    const allSelected = ids.every((id) => selection.has(id));
    setSelection(allSelected ? new Set() : new Set(ids));
  };

  const handleBulkDelete = () => {
    removeBulk.mutate([...selection], {
      onSuccess: () => setSelection(new Set()),
    });
    setConfirmBulkDelete(false);
  };

  const isActivity = tab === 'activity';

  // Activity = the full interaction log (all types, recent first). Résumés and
  // Cover Letters are lenses over the same generations list.
  const { data: interactions = [], isLoading, refetch } = useInteractions();
  const { data: generations = [] } = useAiGenerations();

  const q = filter.trim().toLowerCase();

  const counts: Record<DocTab, number> = useMemo(
    () => ({
      resumes: generations.filter((g) => g.resumeText.trim()).length,
      coverLetters: generations.filter((g) => g.coverLetterText.trim()).length,
      activity: interactions.length,
    }),
    [generations, interactions]
  );

  const generationDocs = useMemo(() => {
    const base =
      tab === 'coverLetters'
        ? generations.filter((g) => g.coverLetterText.trim())
        : generations.filter((g) => g.resumeText.trim());
    return q
      ? base.filter(
          (g) =>
            g.jobTitle.toLowerCase().includes(q) ||
            g.companyName.toLowerCase().includes(q) ||
            g.candidateName.toLowerCase().includes(q)
        )
      : base;
  }, [generations, tab, q]);

  const activityRows = useMemo(() => {
    const sorted = [...(interactions as Interaction[])].sort((a, b) => b.timestamp - a.timestamp);
    return q
      ? sorted.filter(
          (r) => r.title.toLowerCase().includes(q) || r.company.toLowerCase().includes(q)
        )
      : sorted;
  }, [interactions, q]);

  return (
    <PageTransition className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-10 py-10">
        <PageHeader
          title={t('resumes.title')}
          subtitle={t('resumes.subtitle')}
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
              {isActivity && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void refetch()}
                  title={t('resumes.open')}
                >
                  <RefreshCw size={12} className={isLoading ? 'animate-spin' : ''} />
                </Button>
              )}
              {!isActivity && generationDocs.length > 0 && (
                <label className="flex cursor-pointer items-center gap-1.5 text-xs text-foreground/50 select-none">
                  <input
                    type="checkbox"
                    checked={
                      generationDocs.length > 0 && generationDocs.every((g) => selection.has(g.id))
                    }
                    onChange={() => toggleSelectAll(generationDocs.map((g) => g.id))}
                    aria-label={t('resumes.select.selectAll')}
                    className="h-4 w-4 cursor-pointer accent-[color:var(--color-brand)] rounded border border-white/20"
                  />
                  {t('resumes.select.selectAll')}
                </label>
              )}
            </div>
          }
        />

        {/* Tabs */}
        <div className="mb-5 flex items-center gap-1">
          {DOC_TABS.map(({ id, labelKey, icon: Icon, color }) => (
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
              {counts[id] > 0 && (
                <span className="rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] text-foreground/60">
                  {counts[id]}
                </span>
              )}
            </Button>
          ))}
        </div>

        {/* Bulk-action bar — appears when ≥1 generation is selected on a doc tab */}
        <AnimatePresence initial={false}>
          {!isActivity && selection.size > 0 && (
            <motion.div
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={transition.fast}
              className="mb-4 flex items-center justify-between rounded-xl border border-red-400/20 bg-red-400/[0.06] px-4 py-2.5"
            >
              <span className="text-xs text-foreground/60">
                {t('resumes.select.count', { count: selection.size })}
              </span>
              <div className="flex items-center gap-2">
                <Button
                  onClick={() => setSelection(new Set())}
                  className="h-auto rounded-lg border-transparent bg-transparent px-3 py-1.5 text-xs text-foreground/50 hover:text-foreground"
                >
                  {t('resumes.select.clear')}
                </Button>
                <Button
                  onClick={() => setConfirmBulkDelete(true)}
                  disabled={removeBulk.isPending}
                  className="flex h-auto items-center gap-1.5 rounded-lg border-red-400/20 bg-red-400/10 px-3 py-1.5 text-xs text-red-300 hover:bg-red-400/20"
                >
                  <Trash2 size={11} />
                  {t('resumes.select.deleteSelected')}
                </Button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Activity tab — the job-interaction log */}
        {isActivity ? (
          isLoading ? (
            <div className="space-y-2">
              <CardSkeleton /> <CardSkeleton /> <CardSkeleton />
            </div>
          ) : activityRows.length === 0 ? (
            <EmptyState
              icon={Activity}
              title={filter ? t('resumes.noResults') : t('resumes.activity.empty')}
              description={!filter ? t('resumes.activity.emptyDesc') : undefined}
            />
          ) : (
            <motion.div
              className="flex flex-col gap-2"
              variants={stagger.container}
              initial="hidden"
              animate="show"
            >
              <AnimatePresence initial={false}>
                {activityRows.map((row) => (
                  <motion.div
                    key={`${row.jobId}-${row.interactionType}`}
                    variants={stagger.item}
                    transition={transition.normal}
                    exit={{ opacity: 0, y: -6 }}
                  >
                    <InteractionRow row={row} />
                  </motion.div>
                ))}
              </AnimatePresence>
            </motion.div>
          )
        ) : /* Résumés / Cover Letters — lenses over the generations list */
        generationDocs.length === 0 ? (
          <EmptyState
            icon={tab === 'coverLetters' ? Mail : Wand2}
            title={
              filter
                ? t('resumes.noResults')
                : tab === 'coverLetters'
                  ? t('resumes.coverLettersEmpty')
                  : t('resumes.generated.noGenerationsYet')
            }
            description={
              filter
                ? undefined
                : tab === 'coverLetters'
                  ? t('resumes.coverLettersEmptyDesc')
                  : t('resumes.generated.noGenerationsDesc')
            }
          />
        ) : (
          <motion.div
            className="flex flex-col gap-3"
            variants={stagger.container}
            initial="hidden"
            animate="show"
          >
            <AnimatePresence initial={false}>
              {generationDocs.map((gen) => (
                <motion.div
                  key={gen.id}
                  variants={stagger.item}
                  transition={transition.normal}
                  exit={{ opacity: 0, y: -6 }}
                >
                  <GenerationCard
                    gen={gen}
                    selected={selection.has(gen.id)}
                    onToggleSelect={toggleSelect}
                  />
                </motion.div>
              ))}
            </AnimatePresence>
          </motion.div>
        )}
      </div>

      <ConfirmModal
        open={confirmBulkDelete}
        onClose={() => setConfirmBulkDelete(false)}
        onConfirm={handleBulkDelete}
        title={t('resumes.select.confirmTitle')}
        description={t('resumes.select.confirmBody', { count: selection.size })}
        confirmText={t('resumes.select.deleteSelected')}
        variant="danger"
        isConfirming={removeBulk.isPending}
      />
    </PageTransition>
  );
}

export { ResumesPage };
