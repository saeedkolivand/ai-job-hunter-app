/** Token-budget truncation strategies + the smart truncation engine. */

import { detectSections, type ResumeSection } from './sections.js';
import { estimatePages, estimateTokens } from './tokens.js';

export interface TruncationStrategy {
  maxTokens: number;
  preserveSections: string[]; // Section names to always keep
  summarizeSections: string[]; // Section names to summarize if too long
  dropSections: string[]; // Section names to drop if needed
  modelType?: 'large' | 'medium' | 'small'; // Model size hint
}

/** Strategy for large models (GPT-4, Claude, Gemini). Context window: 8K-128K. */
export const LARGE_MODEL_STRATEGY: TruncationStrategy = {
  maxTokens: 6000, // Conservative limit
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills', 'Education'],
  summarizeSections: ['Projects', 'Certifications'],
  dropSections: ['Interests', 'Volunteer', 'Languages', 'Awards', 'Publications'],
  modelType: 'large',
};

/** Strategy for medium models (Llama 3 8B, Mistral 7B). Context window: 4K-8K. */
export const MEDIUM_MODEL_STRATEGY: TruncationStrategy = {
  maxTokens: 3500, // Leave room for job ad + prompt
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills'],
  summarizeSections: ['Education', 'Certifications'],
  dropSections: ['Projects', 'Interests', 'Volunteer', 'Languages', 'Awards', 'Publications'],
  modelType: 'medium',
};

/** Strategy for small local models (Llama 3.2 1B-3B, Phi-3, Gemma 2B). 2K-4K. */
export const SMALL_MODEL_STRATEGY: TruncationStrategy = {
  maxTokens: 1800, // Very aggressive - leave room for prompt + job ad
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills'],
  summarizeSections: [], // No summarization - too expensive for small models
  dropSections: [
    'Education',
    'Projects',
    'Certifications',
    'Interests',
    'Volunteer',
    'Languages',
    'Awards',
    'Publications',
  ],
  modelType: 'small',
};

/** Strategy for resume generation (needs more context). */
export const GENERATION_STRATEGY: TruncationStrategy = {
  maxTokens: 5000,
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills', 'Education'],
  summarizeSections: ['Projects', 'Certifications', 'Awards'],
  dropSections: ['Interests', 'Volunteer', 'Publications'],
  modelType: 'large',
};

/**
 * Truncate experience section intelligently.
 * Keep most recent roles, summarize older ones.
 */
function truncateExperience(content: string, maxTokens: number): string {
  const lines = content.split('\n');
  const roles: string[][] = [];
  let currentRole: string[] = [];

  // Split into individual roles (usually separated by company/title lines)
  for (const line of lines) {
    if (line.trim() === '') {
      if (currentRole.length > 0) {
        roles.push(currentRole);
        currentRole = [];
      }
    } else {
      currentRole.push(line);
    }
  }
  if (currentRole.length > 0) roles.push(currentRole);

  // Keep most recent roles in full, summarize older ones
  const result: string[] = [];
  let currentTokens = 0;

  for (let i = 0; i < roles.length; i++) {
    const role = roles[i];
    if (!role) continue;

    const roleText = role.join('\n');
    const roleTokens = estimateTokens(roleText);

    if (currentTokens + roleTokens <= maxTokens) {
      // Keep full role
      result.push(roleText);
      currentTokens += roleTokens;
    } else if (i < 3) {
      // For first 3 roles, try to keep at least the header
      const header = role.slice(0, 2).join('\n'); // Company + title
      const headerTokens = estimateTokens(header);
      if (currentTokens + headerTokens <= maxTokens) {
        result.push(header + '\n[Details truncated]');
        // Note: currentTokens not used after this point in this branch
      }
      break;
    } else {
      // Older roles: just mention count
      const remaining = roles.length - i;
      result.push(`\n[${remaining} earlier role${remaining > 1 ? 's' : ''} omitted for brevity]`);
      break;
    }
  }

  return result.join('\n\n');
}

/** Summarize a section to reduce token count. */
function summarizeSection(section: ResumeSection): string {
  const targetTokens = Math.floor(section.tokenCount * 0.3); // Reduce to 30%

  if (section.name === 'Experience') {
    return truncateExperience(section.content, targetTokens);
  }

  // For other sections, keep first N lines
  const lines = section.content.split('\n').filter((l) => l.trim());
  const maxLines = Math.max(3, Math.floor(lines.length * 0.3));
  const kept = lines.slice(0, maxLines);
  const omitted = lines.length - maxLines;

  if (omitted > 0) {
    kept.push(`[${omitted} more item${omitted > 1 ? 's' : ''} omitted]`);
  }

  return kept.join('\n');
}

/** Intelligently truncate a resume to fit within a strategy's token limit. */
export function truncateResume(resume: string, strategy: TruncationStrategy): string {
  const sections = detectSections(resume);
  const totalTokens = sections.reduce((sum, s) => sum + s.tokenCount, 0);

  // If already under limit, return as-is
  if (totalTokens <= strategy.maxTokens) {
    return resume;
  }

  const modelType = strategy.modelType || 'large';
  console.warn(
    `Resume too large: ${totalTokens} tokens (limit: ${strategy.maxTokens} for ${modelType} model)`
  );
  console.warn(`Detected ${sections.length} sections, ${estimatePages(resume)} estimated pages`);

  // Build result by priority
  const result: string[] = [];
  let currentTokens = 0;

  // Phase 1: Add all preserve sections
  for (const section of sections) {
    if (strategy.preserveSections.includes(section.name)) {
      if (currentTokens + section.tokenCount <= strategy.maxTokens) {
        result.push(`${section.name.toUpperCase()}\n${section.content}`);
        currentTokens += section.tokenCount;
      } else {
        // Section is too large, need to truncate it.
        const available = strategy.maxTokens - currentTokens;
        if (available <= 0) continue; // No budget left at all — skip.

        const truncated =
          // With a comfortable budget, prefer the section-aware truncators.
          // With a tight budget (<= 100 tokens), those leave nothing useful, so
          // fall back to a hard char-slice so the section is never silently
          // dropped — a positive budget must always yield some content.
          available > 100 && section.name === 'Experience'
            ? truncateExperience(section.content, available)
            : section.content.slice(0, available * 4); // Rough char estimate
        result.push(`${section.name.toUpperCase()}\n${truncated}`);
        currentTokens += estimateTokens(truncated);
      }
    }
  }

  // Phase 2: Add summarize sections if space available
  for (const section of sections) {
    if (strategy.summarizeSections.includes(section.name)) {
      const summarized = summarizeSection(section);
      const summarizedTokens = estimateTokens(summarized);

      if (currentTokens + summarizedTokens <= strategy.maxTokens) {
        result.push(`${section.name.toUpperCase()}\n${summarized}`);
        currentTokens += summarizedTokens;
      }
    }
  }

  // Phase 3: Drop sections are not included

  const finalResume = result.join('\n\n');
  const finalTokens = estimateTokens(finalResume);
  console.warn(
    `Truncated resume: ${finalTokens} tokens (${estimatePages(finalResume)} pages) for ${modelType} model`
  );

  // If still too large (edge case), do hard truncation
  if (finalTokens > strategy.maxTokens) {
    console.warn(`Still too large after smart truncation, applying hard limit`);
    const charLimit = strategy.maxTokens * 4;
    return finalResume.slice(0, charLimit) + '\n\n[Content truncated to fit model limits]';
  }

  return finalResume;
}
