/**
 * `vi.mock` factories for the `useApplicationAnswers` tests. `vi.mock` is hoisted per
 * test file, so each file keeps its own one-line `vi.mock(..., async () => (await
 * import('./mocks')).x)`. This module must NEVER import the hook under test (a value
 * import would deadlock those factories) — the render helpers live in `helpers.ts`.
 */
import { type Mock, vi } from 'vitest';

/** The metadata `extractMetadata` resolves with unless a test overrides it. */
export const META = {
  candidateName: 'Jane',
  jobTitle: 'Engineer',
  companyName: 'Acme',
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  targetLanguage: 'en',
  topRequirements: [],
};

export const save: Mock = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });

// Stub the generation lib: metadata + one deterministic answer, no research.
export const generateModule: Record<string, Mock> = {
  extractMetadata: vi.fn().mockResolvedValue(META),
  generateApplicationAnswer: vi.fn().mockResolvedValue('Because I led a payments migration.'),
  researchCompany: vi.fn().mockResolvedValue(''),
  researchAnswer: vi.fn().mockResolvedValue(''),
  lookupSalaryRange: vi.fn().mockResolvedValue(undefined),
};

export const appClientModule: { useAppClient: () => { aiGenerations: { save: Mock } } } = {
  useAppClient: () => ({ aiGenerations: { save } }),
};
