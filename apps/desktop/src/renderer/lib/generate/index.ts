export { buildFilename, exportDOCX, exportPDF, exportTXT, renderDocumentPreview } from './export';
export {
  type Fabrication,
  type FabricationPresentation,
  parseFabrications,
  presentFabrication,
  unresolvedCount,
} from './fabrications';
export {
  extractMetadata,
  generateApplicationAnswer,
  generateApplicationEmail,
  generateCoverLetter,
  type GeneratedGitHubProject,
  generateGitHubProjects,
  generateInterviewQuestions,
  generateJobAdSummary,
  generateLikelyInterviewQuestions,
  generateReferral,
  generateReferralImprove,
  generateResume,
  generateStarFeedback,
  type GenerationMeta,
  type GenerationMode,
  type LikelyQuestion,
  lookupSalaryRange,
  MODES,
  parseInterviewQuestions,
  parseLikelyQuestions,
  parseStarFeedback,
  researchAnswer,
  researchCompany,
  rewriteSelection,
  type StarCompleteness,
  type StarFeedback,
  synthesizeResume,
} from './generation';
export {
  ALL_GENERATION_DEPTHS,
  type GenerationDepth,
  isDepthRunnable,
  resolveRunnableDepth,
  RUNNABLE_GENERATION_DEPTHS,
} from './generation-depth';
export { buildLinkSuggestions } from './links';
export {
  OUTPUT_LANGUAGES,
  type OutputLanguage,
  safeLocale,
  type SupportedLocale,
  VALID_LOCALES,
} from './locales';
export {
  computeQualityReport,
  hashText,
  mergeRecheckedReport,
  parseQualityReport,
  type QualityReport,
  type QualityReportSlot,
  serializeQualityReport,
} from './quality-report';
export {
  buildSectionVerdicts,
  sectionKeyForHeading,
  type SectionVerdict,
} from './section-verdicts';
export {
  type AtsModeHintKey,
  atsModeHintKey,
  isDesignTier,
  isPhotoTemplate,
  isTwoColumnTemplate,
  LETTER_LAYOUT_IDS,
  type LetterLayoutId,
  TEMPLATE_IDS,
  type TemplateId,
  TEMPLATES,
} from './templates';
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
export type {
  EmphasisId,
  InterviewAudience,
  RewriteDocType,
  SalaryRange,
} from '@ajh/prompts/generate';
export {
  CONNECTION_NOTE_LIMIT,
  EMPHASIS_OPTIONS,
  INTERVIEW_AUDIENCES,
  INTERVIEW_QUESTIONS_PER_AUDIENCE,
  LETTER_MARKET_IDS,
  letterConventions,
  resolveMarket,
} from '@ajh/prompts/generate';
