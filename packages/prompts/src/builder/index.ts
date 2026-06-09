/**
 * Resume Builder prompts (#1 / phase B9) — synthesize a from-scratch résumé from
 * structured interview answers. The `@ajh/prompts/builder` entry point.
 */

export {
  buildBuilderSystemPrompt,
  buildInterviewResumePrompt,
  renderInterviewAnswers,
} from './builder-prompt.js';
export * from './types.js';
