/**
 * Shared mocks + fixtures for the `useReferralDraft` tests (`useReferralDraft.test.ts`,
 * `useReferralDraft.improve.test.ts`). `vi.mock` is hoisted per file, so each test
 * file keeps its own one-line `vi.mock(..., async () => (await import(...)).x)`. This module is
 * what those factories import, so it must NEVER import the hook under test (a value import would
 * deadlock the factory) — the render helpers live in `draft.test-helpers.ts`.
 */
import { vi } from 'vitest';

import type { ReferralChannel } from '@ajh/shared/ipc';

// generateReferral + generateReferralImprove are mocked as vi.fn that resolve with
// a deterministic string. The mock is reset between tests so individual cases can
// override the return value.
type ReferralCallParams = {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  format: ReferralChannel;
  charLimit?: number;
  model: string;
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
};

export const mockGenerateReferral = vi.fn<(params: ReferralCallParams) => Promise<string>>();

export const mockGenerateReferralImprove =
  vi.fn<(params: ReferralCallParams & { draft: string; instruction: string }) => Promise<string>>();

export const generateModule = {
  generateReferral: (...args: Parameters<typeof mockGenerateReferral>) =>
    mockGenerateReferral(...args),
  generateReferralImprove: (...args: Parameters<typeof mockGenerateReferralImprove>) =>
    mockGenerateReferralImprove(...args),
  // CONNECTION_NOTE_LIMIT is a re-export from @ajh/prompts — provide the real value.
  CONNECTION_NOTE_LIMIT: 300,
};

export const languageDetectionModule = {
  detectLanguages: vi.fn<(resume: string, jobAd: string) => { resumeName: string }>(() => ({
    resumeName: 'en',
  })),
};

export const BASE = {
  personName: 'Bob Chen',
  personRole: 'Director',
  companyName: 'Acme',
  jobTitle: 'Senior Engineer',
  resume: 'Jane Doe\nSenior Engineer with 8 years experience.',
  channel: 'linkedin_message' as ReferralChannel,
  model: 'llama3',
  canUse: true,
};

/** Call in `beforeEach`. */
export function resetGenerateMocks() {
  mockGenerateReferral.mockResolvedValue('Hi Bob, I wanted to reach out about the role at Acme.');
  mockGenerateReferralImprove.mockResolvedValue(
    'Hi Bob! I really wanted to reach out about the role at Acme.'
  );
}
