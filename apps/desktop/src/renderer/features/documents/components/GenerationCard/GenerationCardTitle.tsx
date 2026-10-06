import { Building2, Calendar, Check, ChevronDown, Wand2 } from 'lucide-react';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';

interface GenerationCardTitleProps {
  gen: AiGenerationRecord;
  expanded: boolean;
  onToggle: () => void;
}

/** The card's header toggle: icon, title/company, and the meta chips. */
export function GenerationCardTitle({ gen, expanded, onToggle }: GenerationCardTitleProps) {
  const { t } = useTranslation();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const generatedDate = new Date(gen.createdAt).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
  return (
    <Button
      variant="unstyled"
      onClick={onToggle}
      aria-expanded={expanded}
      className="flex min-w-0 flex-1 items-start gap-4 text-left"
    >
      <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-brand/10">
        <Wand2 size={16} className="text-brand-soft" />
      </span>

      <span className="min-w-0 flex-1 space-y-2">
        <span className="block min-w-0">
          <span className="block truncate text-[15px] font-semibold leading-tight text-foreground/90">
            {gen.jobTitle || t('resumes.unknownPosition')}
          </span>
          {gen.companyName && (
            <span className="mt-1 flex items-center gap-1.5 truncate text-xs text-foreground/55">
              <Building2 size={11} className="shrink-0 text-foreground/35" />
              {gen.companyName}
            </span>
          )}
        </span>

        <span className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-foreground/50">
          {gen.candidateName && <span>{gen.candidateName}</span>}
          <span className="flex items-center gap-1 text-foreground/40">
            <Calendar size={11} />
            <span title={formatRelative(gen.createdAt)}>{generatedDate}</span>
          </span>
          <span className="rounded-full border border-brand/20 bg-brand/8 px-2 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
            {gen.mode}
          </span>
          {gen.board && (
            <span className="rounded-full border border-[var(--border-clear)] bg-muted px-2 py-0.5 text-[9px] uppercase tracking-wider text-foreground/55">
              {t(`jobs.boards.${gen.board}`, { defaultValue: gen.board })}
            </span>
          )}
          {/* A linked job means this generation was an application. */}
          {gen.jobUrl && (
            <span className="flex items-center gap-1 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-2 py-0.5 text-[9px] uppercase tracking-wider text-emerald-300">
              <Check size={9} /> {t('resumes.generated.applied')}
            </span>
          )}
        </span>
      </span>

      <ChevronDown
        size={16}
        className={cn(
          'mt-1 shrink-0 text-foreground/30 transition-transform',
          expanded && 'rotate-180'
        )}
      />
    </Button>
  );
}
