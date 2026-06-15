import { ExternalLink as ExternalLinkIcon, Loader2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { cn } from '@ajh/ui';

import { ExternalLink } from '@/components/ui/ExternalLink';

interface Props {
  jobDesc: string;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
  fill?: boolean;
}

export function JobDescriptionPanel({ jobDesc, hasDesc, fetchingDesc, jobUrl, fill }: Props) {
  const { t } = useTranslation();

  return (
    <div className={cn(fill && 'flex min-h-0 flex-1 flex-col')}>
      <div
        className={cn(
          fill && 'shrink-0',
          'mb-1 text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55'
        )}
      >
        {t('autopilot.apply.jobDescription')}
      </div>
      {fetchingDesc ? (
        <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] text-foreground/40">
          <Loader2 size={12} className="animate-spin" />
          {t('autopilot.apply.fetchingDescription')}
        </div>
      ) : hasDesc ? (
        <div
          className={cn(
            'select-text overflow-y-auto whitespace-pre-wrap rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/60',
            fill ? 'flex-1' : 'max-h-32'
          )}
        >
          {jobDesc}
        </div>
      ) : jobUrl ? (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
          {t('autopilot.apply.loadFailed')}{' '}
          <ExternalLink
            href={jobUrl}
            className="inline-flex items-center gap-0.5 font-medium text-brand-soft hover:underline"
          >
            {t('autopilot.viewJob')}
            <ExternalLinkIcon size={10} />
          </ExternalLink>
        </div>
      ) : (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
          {t('autopilot.apply.noDescription')}
        </div>
      )}
    </div>
  );
}
