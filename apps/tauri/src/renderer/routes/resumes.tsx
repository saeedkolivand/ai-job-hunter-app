import {
  Bookmark,
  Building2,
  Calendar,
  ChevronDown,
  Clock,
  Copy,
  Download,
  ExternalLink,
  Eye,
  FileText,
  Loader2,
  MapPin,
  RefreshCw,
  Search,
  Send,
  Trash2,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { Button, CardSkeleton, EmptyState, Input } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { cn } from '@/lib/cn';
import {
  buildFilename,
  exportDOCX,
  exportPDF,
  exportTXT,
  type TemplateId,
} from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';
import { stagger, transition } from '@/lib/motion';
import { useAiGenerations, useRemoveAiGeneration } from '@/services/use-ai-generations';
import { useInteractions } from '@/services/use-postings';
import { useOpenExternal } from '@/services/use-system';
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

function formatRelative(ts: number, t: ReturnType<typeof useTranslation>['t']): string {
  const diff = Date.now() - ts;
  const m = Math.floor(diff / 60000);
  const h = Math.floor(m / 60);
  const d = Math.floor(h / 24);
  if (m < 1) return t('resumes.relativeTime.justNow');
  if (m < 60) return t('resumes.relativeTime.minutesAgo', { m });
  if (h < 24) return t('resumes.relativeTime.hoursAgo', { h });
  if (d < 7) return t('resumes.relativeTime.daysAgo', { d });
  return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

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

function InteractionRow({
  row,
  tabCfg,
}: {
  row: Interaction;
  tabCfg: (typeof TAB_CONFIG)[number];
}) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const Icon = tabCfg.icon;

  return (
    <div className="glass-graphite glass-highlight flex items-center gap-4 rounded-xl p-4 transition-colors hover:bg-white/[0.02]">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/5 text-[10px] uppercase tracking-wider text-brand-soft">
        {row.source.slice(0, 2)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-medium text-foreground/90">
          <span className="truncate">{row.title || t('resumes.unknownPosition')}</span>
          <span
            className={cn(
              'flex shrink-0 items-center gap-1 rounded-full border px-1.5 py-0.5 text-[9px] uppercase tracking-wider',
              tabCfg.ringColor,
              tabCfg.color
            )}
          >
            <Icon size={8} /> {t(tabCfg.labelKey)}
          </span>
        </div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-foreground/50">
          {row.company && (
            <span className="flex items-center gap-1">
              <Building2 size={10} /> {row.company}
            </span>
          )}
          {row.location && (
            <span className="flex items-center gap-1">
              <MapPin size={10} /> {row.location}
            </span>
          )}
          <span className="flex items-center gap-1 text-foreground/35">
            <Clock size={10} /> {formatRelative(row.timestamp, t)}
          </span>
          <span className="text-foreground/30">{row.source}</span>
        </div>
      </div>

      {row.url && (
        <Button
          onClick={() => void openExternal.mutate(row.url)}
          className="flex shrink-0 items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
        >
          <ExternalLink size={11} /> {t('resumes.open')}
        </Button>
      )}
    </div>
  );
}

const EXPORT_FORMATS = ['pdf', 'docx', 'txt'] as const;
type ExportFormat = (typeof EXPORT_FORMATS)[number];

const TEMPLATE_OPTIONS: { id: TemplateId; label: string }[] = [
  { id: 'modern', label: 'Modern' },
  { id: 'classic', label: 'Classic' },
  { id: 'executive', label: 'Executive' },
];

function GenerationCard({ gen }: { gen: AiGenerationRecord }) {
  const { t } = useTranslation();
  const removeAiGeneration = useRemoveAiGeneration();
  const [expanded, setExpanded] = useState<'resume' | 'cover' | 'jobAd' | null>(null);
  const [copied, setCopied] = useState<'resume' | 'cover' | null>(null);
  const [exportFormat, setExportFormat] = useState<ExportFormat>('pdf');
  const [exportTemplate, setExportTemplate] = useState<TemplateId>('modern');
  const [exporting, setExporting] = useState<'resume' | 'cover' | null>(null);

  const copy = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? gen.resumeText : gen.coverLetterText;
    await navigator.clipboard.writeText(text);
    setCopied(type);
    setTimeout(() => setCopied(null), 1800);
  };

  const meta = {
    candidateName: gen.candidateName,
    jobTitle: gen.jobTitle,
    companyName: gen.companyName,
    resumeLanguage: gen.resumeLanguage,
    jobAdLanguage: gen.jobAdLanguage,
    targetLanguage: gen.targetLanguage,
    topRequirements: gen.topRequirements,
    mismatch: gen.mismatch,
  };

  const doExport = async (type: 'resume' | 'cover') => {
    const text = type === 'resume' ? gen.resumeText : gen.coverLetterText;
    if (!text) return;
    const docType = type === 'resume' ? 'resume' : 'cover-letter';
    const filename = buildFilename(meta, docType, exportFormat);
    setExporting(type);
    try {
      if (exportFormat === 'pdf') {
        await exportPDF(text, filename.replace('.pdf', ''), docType, meta, exportTemplate);
      } else if (exportFormat === 'docx') {
        await exportDOCX(text, filename.replace('.docx', ''), docType, meta, exportTemplate);
      } else {
        exportTXT(text, filename.replace('.txt', ''));
      }
    } finally {
      setExporting(null);
    }
  };

  const generatedDate = new Date(gen.createdAt).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });

  return (
    <div className="glass-graphite glass-highlight rounded-xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-4 p-4">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand/10">
          <Wand2 size={14} className="text-brand-soft" />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-sm font-medium text-foreground/90">
            <span className="truncate">{gen.jobTitle || t('resumes.unknownPosition')}</span>
            {gen.companyName && (
              <span className="text-foreground/40 text-xs font-normal shrink-0">
                @ {gen.companyName}
              </span>
            )}
          </div>
          <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-foreground/50">
            {gen.candidateName && <span>{gen.candidateName}</span>}
            <span className="flex items-center gap-1 text-foreground/35">
              <Calendar size={10} />
              <span title={formatRelative(gen.createdAt, t)}>{generatedDate}</span>
            </span>
            <span className="rounded-full border border-brand/20 bg-brand/8 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
              {gen.mode}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {gen.resumeText && (
            <Button
              onClick={() => void copy('resume')}
              className="flex items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
            >
              <Copy size={10} />
              {copied === 'resume'
                ? t('resumes.generated.copied')
                : t('resumes.generated.copyResume')}
            </Button>
          )}
          {gen.coverLetterText && (
            <Button
              onClick={() => void copy('cover')}
              className="flex items-center gap-1 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto border-transparent"
            >
              <Copy size={10} />
              {copied === 'cover'
                ? t('resumes.generated.copied')
                : t('resumes.generated.copyCoverLetter')}
            </Button>
          )}
          <Button
            onClick={() => void removeAiGeneration.mutate(gen.id)}
            className="flex items-center gap-1 rounded-lg bg-white/5 px-2 py-1.5 text-[11px] text-foreground/40 transition-colors hover:text-red-400 h-auto border-transparent"
          >
            <Trash2 size={10} />
          </Button>
        </div>
      </div>

      {/* Export bar */}
      {(gen.resumeText || gen.coverLetterText) && (
        <div className="border-t border-white/[0.04] px-4 py-2.5 flex items-center gap-2 flex-wrap">
          <Download size={11} className="text-foreground/30 shrink-0" />
          <span className="text-[11px] text-foreground/40 mr-1">
            {t('resumes.generated.export')}
          </span>

          {/* Format picker */}
          <div className="flex items-center gap-0.5 rounded-lg border border-white/[0.06] bg-white/[0.03] p-0.5">
            {EXPORT_FORMATS.map((fmt) => (
              <button
                key={fmt}
                onClick={() => setExportFormat(fmt)}
                className={cn(
                  'rounded-md px-2 py-0.5 text-[10px] uppercase tracking-wider transition-colors',
                  exportFormat === fmt
                    ? 'bg-white/10 text-foreground/80'
                    : 'text-foreground/35 hover:text-foreground/60'
                )}
              >
                {fmt}
              </button>
            ))}
          </div>

          {/* Template picker — only for pdf/docx */}
          {exportFormat !== 'txt' && (
            <div className="flex items-center gap-0.5 rounded-lg border border-white/[0.06] bg-white/[0.03] p-0.5">
              {TEMPLATE_OPTIONS.map(({ id, label }) => (
                <button
                  key={id}
                  onClick={() => setExportTemplate(id)}
                  className={cn(
                    'rounded-md px-2 py-0.5 text-[10px] transition-colors',
                    exportTemplate === id
                      ? 'bg-white/10 text-foreground/80'
                      : 'text-foreground/35 hover:text-foreground/60'
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          )}

          <div className="flex items-center gap-1 ml-auto">
            {gen.resumeText && (
              <Button
                disabled={exporting === 'resume'}
                onClick={() => void doExport('resume')}
                className="flex items-center gap-1 rounded-lg bg-brand/10 border-brand/20 px-2.5 py-1.5 text-[11px] text-brand-soft transition-colors hover:bg-brand/20 h-auto"
              >
                {exporting === 'resume' ? (
                  <Loader2 size={10} className="animate-spin" />
                ) : (
                  <Download size={10} />
                )}
                {t('resumes.generated.exportResume')}
              </Button>
            )}
            {gen.coverLetterText && (
              <Button
                disabled={exporting === 'cover'}
                onClick={() => void doExport('cover')}
                className="flex items-center gap-1 rounded-lg bg-white/5 border-white/[0.06] px-2.5 py-1.5 text-[11px] text-foreground/60 transition-colors hover:text-foreground h-auto"
              >
                {exporting === 'cover' ? (
                  <Loader2 size={10} className="animate-spin" />
                ) : (
                  <Download size={10} />
                )}
                {t('resumes.generated.exportCoverLetter')}
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Top requirements */}
      {gen.topRequirements.length > 0 && (
        <div className="px-4 pb-3 flex flex-wrap gap-1">
          {gen.topRequirements.slice(0, 8).map((req) => (
            <span
              key={req}
              className="rounded-full border border-white/[0.06] bg-white/[0.03] px-2 py-0.5 text-[10px] text-foreground/50"
            >
              {req}
            </span>
          ))}
        </div>
      )}

      {/* Expandable sections */}
      {[
        {
          key: 'resume' as const,
          label: t('resumes.generated.resume'),
          text: gen.resumeText,
          icon: FileText,
        },
        {
          key: 'cover' as const,
          label: t('resumes.generated.coverLetter'),
          text: gen.coverLetterText,
          icon: FileText,
        },
        {
          key: 'jobAd' as const,
          label: t('resumes.generated.jobAd'),
          text: gen.jobAd,
          icon: Building2,
        },
      ]
        .filter((s) => s.text)
        .map(({ key, label, text, icon: SectionIcon }) => (
          <div key={key} className="border-t border-white/[0.04]">
            <button
              onClick={() => setExpanded(expanded === key ? null : key)}
              className="flex w-full items-center justify-between px-4 py-2.5 text-left text-xs text-foreground/50 hover:text-foreground/70 transition-colors"
            >
              <span className="flex items-center gap-1.5">
                <SectionIcon size={11} /> {label}
              </span>
              <ChevronDown
                size={12}
                className={cn('transition-transform', expanded === key && 'rotate-180')}
              />
            </button>
            <AnimatePresence initial={false}>
              {expanded === key && (
                <motion.div
                  initial={{ height: 0 }}
                  animate={{ height: 'auto' }}
                  exit={{ height: 0 }}
                  transition={transition.normal}
                  className="overflow-hidden"
                >
                  <pre className="px-4 pb-4 whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-foreground/50 max-h-64 overflow-y-auto">
                    {text}
                  </pre>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        ))}
    </div>
  );
}
