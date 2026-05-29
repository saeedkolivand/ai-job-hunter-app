import { ExternalLink, Loader2 } from 'lucide-react';

import { useTranslation } from '@/lib/i18n';
import { useOpenExternal } from '@/services';

interface Props {
  jobDesc: string;
  hasDesc: boolean;
  fetchingDesc: boolean;
  jobUrl?: string;
}

export function JobDescriptionPanel({ jobDesc, hasDesc, fetchingDesc, jobUrl }: Props) {
  const { t } = useTranslation();
  const openExternal = useOpenExternal();

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
        <div className="max-h-32 overflow-y-auto whitespace-pre-wrap rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[11px] leading-relaxed text-foreground/60">
          {jobDesc}
        </div>
      ) : jobUrl ? (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
          {t('autopilot.apply.loadFailed')}{' '}
          <button
            type="button"
            onClick={() => void openExternal.mutate(jobUrl)}
            className="inline-flex items-center gap-0.5 font-medium text-brand-soft hover:underline"
          >
            {t('autopilot.viewJob')}
            <ExternalLink size={10} />
          </button>
        </div>
      ) : (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200/80">
          {t('autopilot.apply.noDescription')}
        </div>
      )}
    </div>
  );
}
