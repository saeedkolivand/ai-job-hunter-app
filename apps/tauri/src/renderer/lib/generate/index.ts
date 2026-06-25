export { buildFilename, exportDOCX, exportPDF, exportTXT, renderDocumentPreview } from './export';
export {
  extractMetadata,
  generateApplicationAnswer,
  generateCoverLetter,
  type GeneratedGitHubProject,
  generateGitHubProjects,
  generateInterviewQuestions,
  generateJobAdSummary,
  generateReferral,
  generateReferralImprove,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  MODES,
  parseInterviewQuestions,
  researchCompany,
  rewriteSelection,
  synthesizeResume,
} from './generation';
export { buildLinkSuggestions } from './links';
export {
  OUTPUT_LANGUAGES,
  type OutputLanguage,
  safeLocale,
  type SupportedLocale,
  VALID_LOCALES,
} from './locales';
export { isTwoColumnTemplate, TEMPLATE_IDS, type TemplateId, TEMPLATES } from './templates';
export type {
  InterviewAnswers,
  InterviewEducation,
  InterviewEntry,
  InterviewExperience,
  InterviewProject,
  InterviewPublication,
} from '@ajh/prompts/builder';

/**
 * Debounce window (ms) for persisting an in-progress inline edit to a saved
 * generation. Shared so the two edit surfaces (GenerationCard, ApplyPage's
 * useTailorGeneration) stay in lockstep.
 */
export const PERSIST_DEBOUNCE_MS = 800;
export type { EmphasisId, InterviewAudience, RewriteDocType } from '@ajh/prompts/generate';
export {
  CONNECTION_NOTE_LIMIT,
  EMPHASIS_OPTIONS,
  INTERVIEW_AUDIENCES,
  INTERVIEW_QUESTIONS_PER_AUDIENCE,
  LETTER_MARKET_IDS,
  letterConventions,
  resolveMarket,
} from '@ajh/prompts/generate';
