/**
 * i18n adapter — single point of contact for translation utilities.
 *
 * Components import from here, never directly from "react-i18next".
 * If the library changes its API or is replaced, update this file only.
 *
 * What lives here:
 *   useTranslation  — the primary hook used in every component
 *   TFunction       — type for the `t` function when passed as a prop
 *
 * What stays in i18n/index.ts:
 *   initReactI18next, i18n.init() — library setup, not a component concern
 *
 * ESLint note: this file is explicitly exempted from the no-restricted-imports
 * rule for react-i18next (see eslint.config.mjs) — it IS the exception point.
 */
export { useTranslation } from 'react-i18next';

// TFunction lives in i18next (the core), not react-i18next
export type { TFunction } from 'i18next';
