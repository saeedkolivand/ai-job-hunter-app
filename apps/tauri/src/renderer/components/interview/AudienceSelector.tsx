import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { INTERVIEW_AUDIENCES } from '@/lib/generate';

interface Props {
  /** Currently-selected audience ids. */
  selected: string[];
  /** Toggle one audience id on/off. */
  onToggle: (audience: string) => void;
}

/**
 * Multi-select chips for the interviewer audiences the user wants questions for
 * (recruiter/HR, hiring manager, team, leadership, general). Shared by the
 * Interview-prep tab and the apply-time modal so the taxonomy + labels live once.
 * The audience set is the canonical {@link INTERVIEW_AUDIENCES} from the prompt layer.
 */
export function AudienceSelector({ selected, onToggle }: Props) {
  const { t } = useTranslation();

  return (
    <div
      className="flex flex-wrap gap-1.5"
      role="group"
      aria-label={t('applications.detail.interview.audienceLabel')}
    >
      {INTERVIEW_AUDIENCES.map((aud) => {
        const on = selected.includes(aud);
        return (
          <Button
            key={aud}
            variant="unstyled"
            type="button"
            aria-pressed={on}
            onClick={() => onToggle(aud)}
            className={cn(
              'rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors',
              on
                ? 'border-brand/40 bg-brand/15 text-brand-soft'
                : 'border-white/[0.08] bg-white/[0.02] text-foreground/50 hover:text-foreground/80'
            )}
          >
            {t(`applications.detail.interview.audience.${aud}`)}
          </Button>
        );
      })}
    </div>
  );
}
