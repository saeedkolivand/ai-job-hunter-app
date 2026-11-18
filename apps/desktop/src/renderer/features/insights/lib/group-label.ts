import type { InsightsDimension } from '@ajh/shared';
import type { TFunction } from '@ajh/translations';

import { TEMPLATES } from '@/lib/generate';

import { UNKNOWN_KEY } from './insights';

/** Language is recorded on a generation, so its "unknown" covers both no generation
 * and a generation with no target language. A template is only recorded on an in-app export with the job link, so its "unknown"
 * also covers tailored applications exported earlier or elsewhere. */
const UNKNOWN_COPY: Partial<Record<InsightsDimension, { label: string; hint: string }>> = {
  targetLanguage: {
    label: 'insights.group.languageNotRecorded',
    hint: 'insights.group.languageNotRecordedHint',
  },
  template: {
    label: 'insights.group.templateNotRecorded',
    hint: 'insights.group.templateNotRecordedHint',
  },
};

/** A muted one-line explanation for a group, when its label alone is not enough. */
export function groupHint(t: TFunction, dimension: InsightsDimension, key: string): string | null {
  const copy = key === UNKNOWN_KEY ? UNKNOWN_COPY[dimension] : undefined;
  return copy ? t(copy.hint) : null;
}

/** A human label for one group key; unknown values fall back to the raw key. */
export function groupLabel(
  t: TFunction,
  language: string,
  dimension: InsightsDimension,
  key: string
): string {
  if (key === UNKNOWN_KEY) return t(UNKNOWN_COPY[dimension]?.label ?? 'insights.group.unknown');
  switch (dimension) {
    case 'tailored':
    case 'coverLetter':
      return t(`insights.group.${dimension}.${key}`);
    case 'month':
      return new Date(`${key}-01T00:00:00Z`).toLocaleDateString(language, {
        month: 'long',
        year: 'numeric',
        timeZone: 'UTC',
      });
    case 'targetLanguage':
      try {
        return new Intl.DisplayNames([language], { type: 'language' }).of(key) ?? key;
      } catch {
        return key; // not a well-formed language code
      }
    case 'template':
      return Object.hasOwn(TEMPLATES, key) ? TEMPLATES[key as keyof typeof TEMPLATES].name : key;
    default:
      return key;
  }
}
