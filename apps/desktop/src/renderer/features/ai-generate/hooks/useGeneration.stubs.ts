import { vi } from 'vitest';

// Factory body for the `vi.mock('@/lib/generate')` in the useGeneration suites. Kept
// apart from ./useGeneration.test-support (which imports the subject) so the lazily
// loaded factory never waits on the module that is itself waiting on the mock.
// extractMetadata resolves a minimal meta; the résumé/cover generators return a
// fixed string by default.
export function generateMock(): Record<string, unknown> {
  return {
    extractMetadata: vi.fn().mockResolvedValue({
      candidateName: 'A',
      jobTitle: 'Dev',
      companyName: 'Co',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    }),
    generateResume: vi.fn(async (..._a: unknown[]) => 'RESUME'),
    generateCoverLetter: vi.fn(async (..._a: unknown[]) => ({
      text: 'COVER',
      companyBrief: 'BRIEF',
    })),
    computeQualityReport: vi.fn().mockResolvedValue(null),
    serializeQualityReport: vi.fn((r: unknown) => (r ? JSON.stringify(r) : undefined)),
  };
}
