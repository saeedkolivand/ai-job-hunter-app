import type { AutopilotBestMatch } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Tag } from '@ajh/ui';

import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';

/**
 * Two facts a recommendation must not blur (#1447, #1444): whether the user already has an
 * application (this exact posting vs only a related copy) and whether the posting was verified
 * still open. Both are reported by the backend; this only labels them.
 */
export function BestMatchStatusChips({ match }: { match: AutopilotBestMatch }) {
  const { t } = useTranslation();
  const formatRelativeTime = useFormatRelativeTime(t);

  // `applied` alone (an older backend) reads as exact: never weaker than before.
  const appliedLabel = match.appliedExact
    ? 'bestMatches.row.appliedExact'
    : match.appliedRelated
      ? 'bestMatches.row.appliedRelated'
      : match.applied
        ? 'bestMatches.row.appliedExact'
        : null;

  const checkedAt = match.availabilityCheckedAt;
  const verified = match.availability === 'open';

  return (
    <>
      {appliedLabel && (
        <Tag color="default" className="px-1.5 py-0 text-[9px]">
          {t(appliedLabel)}
        </Tag>
      )}
      <Tag color={verified ? 'default' : undefined} className="px-1.5 py-0 text-[9px]">
        {verified && checkedAt
          ? t('bestMatches.row.availabilityOpen', { time: formatRelativeTime(checkedAt) })
          : checkedAt
            ? t('bestMatches.row.availabilityUnknown', { time: formatRelativeTime(checkedAt) })
            : t('bestMatches.row.availabilityUnchecked')}
      </Tag>
    </>
  );
}
