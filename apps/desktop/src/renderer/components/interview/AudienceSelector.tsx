import { useTranslation } from '@ajh/translations';
import { Tag } from '@ajh/ui';

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
      {INTERVIEW_AUDIENCES.map((aud) => (
        <Tag.CheckableTag key={aud} checked={selected.includes(aud)} onChange={() => onToggle(aud)}>
          {t(`applications.detail.interview.audience.${aud}`)}
        </Tag.CheckableTag>
      ))}
    </div>
  );
}
