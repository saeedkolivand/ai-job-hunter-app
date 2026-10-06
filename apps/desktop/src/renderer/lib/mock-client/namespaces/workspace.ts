import type { AppClient } from '../../app-client';
import { emptyList, noop, unsub } from './helpers';

export const workspaceNamespaces = (): Pick<
  AppClient,
  | 'applications'
  | 'documents'
  | 'jobPreferences'
  | 'dedup'
  | 'discovery'
  | 'contactProfile'
  | 'github'
> => ({
  applications: {
    list: emptyList,
    get: async () => ({ application: null, events: [] }),
    setStatus: async () => ({ success: true }),
    acceptStatusEvent: async () => ({ success: true }),
    rejectStatusEvent: async () => ({ success: true }),
    update: async () => ({ success: true }),
    remove: async () => ({ success: true }),
    track: async () => ({ success: true }),
    saveFromPosting: async () => ({ success: true }),
    onChanged: unsub,
  },

  documents: {
    list: emptyList,
    getText: async () => '',
    import: noop,
    recommendTemplate: async () => ({
      templateId: 'classic',
      locale: 'en',
      atsSuggested: false,
      rationale: 'Mock recommendation.',
    }),
    remove: noop,
    setDefault: noop,
    exportDocument: async () => ({ data: [], mimeType: 'text/plain', filename: 'mock.txt' }),
    exportAndSave: noop,
    renderPreviewImages: async () => ({ pages: [], mimeType: 'image/svg+xml' }),
  },

  jobPreferences: {
    get: async () => ({}),
    set: noop,
    setSalaryExpectation: noop,
    setExtraAgencyCompanies: noop,
    setSemanticScoring: noop,
  },

  dedup: {
    markNotDuplicate: async () => ({ success: true }),
  },

  discovery: {
    searchCompanies: emptyList,
    setStarred: async () => ({ success: true }),
    watched: emptyList,
  },

  contactProfile: {
    get: async () => ({}),
    set: async () => ({ success: true }),
    headerLine: async () => '',
  },

  github: {
    importRepos: emptyList,
  },
});
