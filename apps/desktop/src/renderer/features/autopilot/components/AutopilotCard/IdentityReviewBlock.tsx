import type { AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

interface Props {
  job: AutopilotFoundJob;
  /** DOM id the Apply button points at via aria-describedby. */
  describedById: string;
  pending: boolean;
  onDismiss: (job: AutopilotFoundJob) => void;
}

function Side({
  label,
  title,
  company,
  url,
}: Record<'label' | 'title' | 'company' | 'url', string>) {
  return (
    <div className="min-w-0 flex-1">
      <dt className="text-fine-print font-semibold uppercase tracking-wide text-foreground/50">
        {label}
      </dt>
      <dd className="truncate text-[10px] text-foreground/80" title={title}>
        {title}
        {company && ` · ${company}`}
      </dd>
      <dd className="truncate text-[10px] text-foreground/40" title={url}>
        {url}
      </dd>
    </div>
  );
}

/** Held-for-review block (#1445): why Apply is off, stored vs resolved identity, Dismiss. */
export function IdentityReviewBlock({ job, describedById, pending, onDismiss }: Props) {
  const { t } = useTranslation();
  const review = job.identityReview;
  if (!review) return null;
  return (
    <div className="ml-0.5 flex flex-col gap-1.5 rounded-lg border border-amber-400/30 bg-amber-400/10 px-2.5 py-1.5">
      <p id={describedById} className="text-[10px] leading-relaxed text-foreground/70">
        {t('autopilot.identityReview.explain')}
      </p>
      <dl className="flex gap-3">
        <Side
          label={t('autopilot.identityReview.stored')}
          title={job.title}
          company={job.company}
          url={job.url}
        />
        <Side
          label={t('autopilot.identityReview.resolved')}
          title={review.resolvedTitle}
          company={review.resolvedCompany}
          url={review.resolvedUrl}
        />
      </dl>
      <Button
        variant="ghost"
        onClick={() => onDismiss(job)}
        disabled={pending}
        className="h-auto w-fit px-2 py-0.5 text-[10px]"
      >
        {t('autopilot.identityReview.dismiss')}
      </Button>
    </div>
  );
}
