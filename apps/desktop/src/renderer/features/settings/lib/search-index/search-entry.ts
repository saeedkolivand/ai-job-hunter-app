import type { SectionId } from '@/features/settings/constants';

/**
 * One searchable entry per setting control/card.
 *
 * - `titleKey`  — existing i18n key for the label displayed in results
 * - `keywords`  — locale-invariant synonyms searched verbatim
 * - `anchor`    — stable `data-settings-anchor` value on the rendered element
 */
export interface SearchEntry {
  id: string;
  section: SectionId;
  titleKey: string;
  keywords: string[];
  anchor: string;
}
