import type { HelpSearchResult } from '@ajh/shared';

import type { AppClient } from '../../app-client';
import { emptyList, noop, unsub } from './helpers';

export const appNamespaces = (): Pick<
  AppClient,
  | 'updater'
  | 'resume'
  | 'resumePipeline'
  | 'support'
  | 'help'
  | 'autopilot'
  | 'menu'
  | 'notifications'
  | 'dialog'
> => ({
  updater: {
    check: noop,
    download: noop,
    install: noop,
    changelog: () => Promise.resolve({ releases: [] }),
    onStatus: unsub,
  },

  resume: {
    extractText: noop,
    // Canned "clean" report by default — tests that care about issues/severity
    // override `resume.validateContent` per-call via `overrides`.
    validateContent: async () => ({
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: null,
        topRequirementHits: 0,
        duplicateRatio: 0,
        rolesSource: 0,
        rolesOutput: 0,
      },
    }),
  },

  // Staged résumé pipeline. `run` resolves with ids (a test that asserts on a
  // run has to override it anyway), `get` with `null` = "no such run", and
  // `listForJob` with an empty history — the shapes a panel renders before
  // anything has been generated.
  resumePipeline: {
    run: () => Promise.resolve({ runId: 'run-mock', jobId: 'job-mock' }),
    get: () => Promise.resolve(null),
    listForJob: emptyList,
    regenerateSection: noop,
    resolveFabrication: noop,
    onStage: unsub,
  },

  support: {
    exportDiagnostics: noop,
  },

  help: {
    // Same reason `scrape.hybridSearch` above resolves a real shape rather
    // than `noop`: the help chat reads `results`/`mode` off the reply on
    // every question, so an `undefined` would throw before the UI could
    // degrade. Empty results with `keyword` mode is the honest "nothing
    // ranked, and nothing semantic ran" answer.
    search: async (): Promise<HelpSearchResult> => ({
      results: [],
      mode: 'keyword',
      arms: { lexical: 'ran', dense: 'skipped' },
    }),
  },

  autopilot: {
    list: emptyList,
    get: noop,
    create: noop,
    update: noop,
    remove: noop,
    run: noop,
    pause: noop,
    resume: noop,
    onStep: () => () => {},
    onFocus: () => () => {},
    takePendingFocus: () => Promise.resolve(null),
    bestMatches: async () => ({ matches: [], total: 0, autopilotCount: 0 }),
  },

  menu: {
    onNavigate: unsub,
    onAction: unsub,
    takePending: () => Promise.resolve(null),
  },

  notifications: {
    list: emptyList,
    markRead: noop,
    markAllRead: noop,
    remove: noop,
    clearAll: noop,
    clicked: noop,
    onChanged: unsub,
    onOpenInbox: unsub,
    onToast: unsub,
  },

  dialog: {
    openFiles: async () => [] as string[],
  },
});
