/**
 * LLM generation for Resume + Cover Letter (and the surrounding surfaces).
 *
 * 1. Extract metadata (JSON — name, role, company, languages, keywords)
 * 2. Generate resume      (streamed text with **keyword** bold markers)
 * 3. Generate cover letter (streamed text with **keyword** bold markers)
 *
 * Generation runs through the backend orchestration pipeline (`ai.generatePipeline`),
 * which streams `ai:stream` deltas under the returned jobId. Export lives in `../export`.
 * This file is the public entry point; each responsibility lives in a sibling module.
 */

export { generateApplicationAnswer, generateApplicationEmail } from './application';
export { generateCoverLetter } from './cover-letter';
export { type GeneratedGitHubProject, generateGitHubProjects } from './github-projects';
export { seedHeaderFromProfile } from './header-seed';
export {
  generateInterviewQuestions,
  generateLikelyInterviewQuestions,
  generateStarFeedback,
} from './interview';
export { generateHelpAnswer, generateJobAdSummary } from './job-ad-help';
export { extractMetadata } from './metadata';
export { generateReferral, generateReferralImprove } from './referral';
export { lookupSalaryRange, researchAnswer, researchCompany } from './research';
export { generateResume, synthesizeResume } from './resume';
export { resolveRewriteTimeoutMs, rewriteSelection } from './rewrite-selection';
export type { GenerationMeta, GenerationMode } from '@ajh/prompts/generate';
export { MODES } from '@ajh/prompts/generate';
