/**
 * Thin re-export of the shared relative-time formatter, kept at this path for
 * the jobs feature's existing import. The implementation (and the full
 * minute→month tiering) lives in `@/hooks/use-format-relative-time`; this
 * defaults to the `jobs` i18n namespace.
 */
export { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
