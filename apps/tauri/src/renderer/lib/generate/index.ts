export { buildFilename, exportDOCX, exportPDF, exportTXT, renderPdfPreview } from './export';
export {
  extractMetadata,
  generateApplicationAnswer,
  generateCoverLetter,
  generateReferral,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  MODES,
  researchCompany,
  rewriteSelection,
  synthesizeResume,
} from './generation';
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
export type { EmphasisId, RewriteDocType } from '@ajh/prompts/generate';
export {
  CONNECTION_NOTE_LIMIT,
  EMPHASIS_OPTIONS,
  LETTER_MARKET_IDS,
  letterConventions,
  resolveMarket,
} from '@ajh/prompts/generate';
