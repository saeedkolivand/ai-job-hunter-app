export const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
export const MAX_BYTES = 25 * 1024 * 1024;

export const GENERATION_STAGES = [
  'aiGenerate.stages.analyzing',
  'aiGenerate.stages.extracting',
  'aiGenerate.stages.mapping',
  'aiGenerate.stages.optimizing',
  'aiGenerate.stages.rewriting',
  'aiGenerate.stages.adapting',
  'aiGenerate.stages.finalizing',
] as const;
