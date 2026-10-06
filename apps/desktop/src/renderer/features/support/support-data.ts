import { setupSections } from './support-data/setup-sections';
import { troubleshootingSections } from './support-data/troubleshooting-sections';
import type { Section, Translate } from './support-data/types';
import { workflowSections } from './support-data/workflow-sections';

export type { Section };

/**
 * The help corpus (ADR-041): how-to sections first, troubleshooting after.
 *
 * Every `t()` call is written out with a literal key, which is what lets both
 * key readers see it: `i18next-cli` (whose CI step is advisory —
 * `continue-on-error`) and `support-data.test.ts`, which drives this function
 * with a recording `t` and checks each emitted key against the en and de
 * bundles. A computed key would be invisible to both.
 */
export function getSupportSections(t: Translate): Section[] {
  return [...workflowSections(t), ...setupSections(t), ...troubleshootingSections(t)];
}
