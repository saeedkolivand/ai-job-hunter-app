import { type Mock, vi } from 'vitest';

import type { AiGenerationRecord } from '@ajh/shared/ipc';
import type * as AjhUi from '@ajh/ui';

import { _registerClient } from '@/lib/app-client';
import { createMockClient } from '@/lib/mock-client';

import { GenerationCard as CardUnderTest } from './index';

// Shared mocks + fixture for the GenerationCard suites: every `vi.mock` lives here, so a
// suite renders the card exported below, never `./index` directly.

// Mock the service barrel (ExternalLink reaches for useOpenExternal) and the
// remove hook so the card renders without a provider tree. We capture the
// mutate spy to prove deletion only fires *after* confirmation.
export const mockMutate: Mock = vi.fn();
export const mockUpdateMutate: Mock = vi.fn();

// Translation resolver seam. Defaults to identity (raw key) so the delete +
// debounced-persist blocks assert on keys; the board-chip block swaps in the REAL
// en resolver (beforeAll/afterAll) so it verifies actual localized strings.
export const translator: { current: (key: string, opts?: { defaultValue?: string }) => string } = {
  current: (key) => key,
};

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: vi.fn() }),
  // EditableOutput → useSelectedModel now reads the backend active-config query
  // (task #16); stub it so the card mounts without a provider tree.
  useActiveConfig: () => ({
    data: { activeProvider: 'ollama', model: 'llama3', providers: { ollama: { model: 'llama3' } } },
    isPending: false,
  }),
}));

vi.mock('@/services/use-ai-generations', () => ({
  useRemoveAiGeneration: () => ({ mutate: mockMutate, isPending: false }),
  // The card debounce-persists inline edits through this hook; the delete tests
  // don't edit, but the component calls it on render so the mock must provide it.
  useUpdateAiGeneration: () => ({ mutate: mockUpdateMutate, isPending: false }),
}));

// The ATS panel reads the exported file through a service hook; stub it so the
// card mounts without an AppClientProvider (the panel has its own test).
vi.mock('@/components/generation/AtsViewPanel', () => ({
  AtsViewPanel: ({ path }: { path: string | null }) =>
    path ? <div data-testid="ats-panel">{path}</div> : null,
}));

// GenerationCard renders EditableOutput which calls useContactProfile() — stub it
// so the component tree mounts without an AppClientProvider.
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
}));

// The card now display-joins referrals and toasts on copy/mark-sent; stub these
// so it renders without a QueryClient/Notification provider tree.
vi.mock('@/services/use-referrals/use-referrals', () => ({
  useReferrals: () => ({ data: [] }),
  useUpsertReferral: () => ({ mutate: vi.fn(), isPending: false }),
}));

// useNotification: return an API of spies; the export-failure test asserts on it.
export const mockNotify: Record<
  'open' | 'success' | 'error' | 'info' | 'warning' | 'destroy',
  Mock
> = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => mockNotify,
  };
});

// t() delegates to a swappable resolver — identity by default (raw key), the real
// en resolver inside the board-chip block. See `translator` above.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => translator.current(key, opts),
  }),
}));

// The renderer export helpers stay REAL; only the IPC edge is stubbed. Each suite
// sets `exportAndSave`'s result (the saved path, or a rejection) and asserts the
// request + generation id it was called with.
export const exportAndSave: Mock = vi.fn();
_registerClient(createMockClient({ documents: { exportAndSave } }));

export const GEN: AiGenerationRecord = {
  id: 'gen-1',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  candidateName: 'Jane Doe',
  createdAt: Date.now(),
  mode: 'ats',
  board: '',
  jobUrl: 'https://acme.com/jobs/1',
  resumeText: 'Original resume text.',
  coverLetterText: 'Original cover letter text.',
  jobAd: '',
  companyBrief: '',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  targetLanguage: 'en',
  mismatch: false,
  topRequirements: [],
  applicationAnswers: [],
  interviewQuestions: [],
};

export const GenerationCard = CardUnderTest;
