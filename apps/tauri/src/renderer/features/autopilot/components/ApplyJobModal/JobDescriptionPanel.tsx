import { ExternalLink as ExternalLinkIcon, Loader2 } from 'lucide-react';

import { ExternalLink } from '@/components/ui/ExternalLink';
import { useTranslation } from '@/lib/i18n';

interface Props {
  jobDesc: string;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
}

export function JobDescriptionPanel({ jobDesc, hasDesc, fetchingDesc, jobUrl }: Props) {
  const { t } = useTranslation();

  return (
    <div>
      <div className="mb-1 text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/35">
        {t('autopilot.apply.jobDescription')}
      </div>
      {fetchingDesc ? (
        <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] text-foreground/40">
          <Loader2 size={12} className="animate-spin" />
          {t('autopilot.apply.fetchingDescription')}
        </div>
      ) : hasDesc ? (
        <div className="select-text max-h-32 overflow-y-auto whitespace-pre-wrap rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
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
