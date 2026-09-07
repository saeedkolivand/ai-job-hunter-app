/**
 * The sidebar's page list as DATA — routes, `nav.*` label keys and tour ids,
 * with no icons and no React in it.
 *
 * Split out of the component because the list has a SECOND reader: the help
 * chat sends these page names to the model as the app's own shipped copy (the
 * APP PAGES block of ADR-043's prompt), so a question the corpus does not
 * cover can still end at the page the feature lives on. One list rather than
 * two, because a second hand-maintained copy drifts silently — a page missing
 * from what the model sees only makes it abstain, with nothing on screen
 * saying why.
 */

import { ROUTES } from '@/constants/routes';

/** One sidebar page. `labelKey` is a `nav.*` key, never rendered text. */
export interface NavPage {
  to: string;
  labelKey: string;
  tourId: string;
}

/**
 * One sidebar group.
 *
 * `labelKey: null` is the PINNED group the sidebar renders in its footer: it
 * ships no heading, and no `nav.sections.*` string names it, so a consumer
 * renders its pages without a section name rather than inventing copy for one.
 */
export interface NavSection {
  labelKey: string | null;
  pages: readonly NavPage[];
}

/**
 * Every page the sidebar links to, in sidebar order.
 *
 * `as const` so each `tourId` keeps its literal type: the component's icon map
 * is keyed by {@link NavTourId}, which turns "added a page, forgot its icon"
 * into a type error instead of a hole in the nav.
 */
export const SIDEBAR_NAV = [
  {
    labelKey: 'nav.sections.workspace',
    pages: [
      { to: ROUTES.DASHBOARD, labelKey: 'nav.dashboard', tourId: 'dashboard' },
      { to: ROUTES.APPLICATIONS, labelKey: 'nav.applications', tourId: 'applications' },
      { to: ROUTES.JOBS, labelKey: 'nav.jobs', tourId: 'jobs' },
      { to: ROUTES.ANALYZE, labelKey: 'nav.analyze', tourId: 'analyze' },
      { to: ROUTES.GENERATE, labelKey: 'nav.generate', tourId: 'generate' },
      { to: ROUTES.BUILD, labelKey: 'nav.build', tourId: 'build' },
      { to: ROUTES.RESUMES, labelKey: 'nav.documents', tourId: 'documents' },
    ],
  },
  {
    labelKey: 'nav.sections.automation',
    pages: [
      { to: ROUTES.AUTOPILOT, labelKey: 'nav.autopilot', tourId: 'autopilot' },
      { to: ROUTES.BEST_MATCHES, labelKey: 'nav.bestMatches', tourId: 'best-matches' },
      { to: ROUTES.MONITORING, labelKey: 'nav.monitoring', tourId: 'monitoring' },
    ],
  },
  {
    // The footer group — pinned below the sections, with no heading of its own.
    labelKey: null,
    pages: [
      { to: ROUTES.SUPPORT, labelKey: 'nav.support', tourId: 'support' },
      { to: ROUTES.SETTINGS, labelKey: 'nav.settings', tourId: 'settings' },
    ],
  },
] as const satisfies readonly NavSection[];

/** The exact page objects in {@link SIDEBAR_NAV} — literal `tourId`s. */
export type SidebarPage = (typeof SIDEBAR_NAV)[number]['pages'][number];

/** Every `data-tour-id` the sidebar renders. */
export type NavTourId = SidebarPage['tourId'];
