import type { GenerationMeta } from '../modes/index.js';
import type { ApplicationEmailParams } from './application-email.js';

export const RESUME =
  'Jane Doe\nSenior Backend Engineer\nBerlin, Germany | jane@example.com\n\n' +
  'PROFESSIONAL SUMMARY\n' +
  'Eight years building distributed systems in Go and TypeScript.\n\n' +
  'EXPERIENCE\n' +
  'Acme Corp — Staff Engineer (2020–2024)\n' +
  '- Led the migration of the billing platform to microservices, cutting p99 latency by 40%.\n' +
  '- Owned on-call for a service processing 2M transactions per day.\n' +
  'Beta Inc — Senior Engineer (2016–2020)\n' +
  '- Shipped the first real-time analytics dashboard used by 500+ customers.\n\n' +
  'SKILLS\n' +
  'Go, TypeScript, Kubernetes, PostgreSQL, Kafka\n\n' +
  'EDUCATION\n' +
  'BSc Computer Science — University of Berlin (2016)\n';

export const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'Jane Doe',
  jobTitle: 'Senior Backend Engineer',
  companyName: 'Globex',
  targetLanguage: 'en',
  topRequirements: ['Go', 'Kubernetes', 'TypeScript'],
};

export const BASE: ApplicationEmailParams = {
  resume: RESUME,
  jobAd: 'Globex is hiring a Senior Backend Engineer to scale our distributed systems.',
  meta: META,
};

/**
 * One `PromptTarget` per resolved depth (see `resolveProfile`), labelled by the
 * depth it resolves to so an `it.each` failure names the path rather than the
 * provider tier: 'small' -> brief, {kind:'cli'} -> task, 'large' -> full.
 */
export const ALL_DEPTHS = [
  ['brief', 'small'],
  ['task', { kind: 'cli' }],
  ['full', 'large'],
] as const;

// ─── Subject-line contract ─────────────────────────────────────────────────────

const DE_META: GenerationMeta = { ...META, targetLanguage: 'de', mismatch: true };
export const DE_BASE: ApplicationEmailParams = { ...BASE, meta: DE_META, market: 'de' };
