/** Multi-pass analysis + condensation for very large resumes, and stats. */

import { detectSections, type ResumeSection } from './sections.js';
import { estimatePages, estimateTokens } from './tokens.js';

export interface MultiPassResult {
  sections: ResumeSection[];
  summaries: Record<string, string>;
  totalTokens: number;
  strategy: 'single-pass' | 'multi-pass';
}

/** Analyze resume structure and determine if multi-pass is needed. */
export function analyzeResumeSize(resume: string, maxTokens = 6000): MultiPassResult {
  const sections = detectSections(resume);
  const totalTokens = sections.reduce((sum, s) => sum + s.tokenCount, 0);

  if (totalTokens <= maxTokens) {
    return { sections, summaries: {}, totalTokens, strategy: 'single-pass' };
  }

  // Multi-pass needed: create summaries for large sections
  const summaries: Record<string, string> = {};

  for (const section of sections) {
    if (section.tokenCount > 1000 && section.name === 'Experience') {
      // Create a condensed summary of experience
      const roles = section.content.split('\n\n');
      const summary = roles
        .slice(0, 3)
        .map((role) => {
          const lines = role.split('\n').filter((l) => l.trim());
          return lines.slice(0, 2).join(' | '); // Company | Title
        })
        .join('\n');

      summaries[section.name] = summary;
    }
  }

  return { sections, summaries, totalTokens, strategy: 'multi-pass' };
}

/** Create a condensed version of a resume for initial analysis. */
export function createCondensedResume(resume: string): string {
  const analysis = analyzeResumeSize(resume);

  if (analysis.strategy === 'single-pass') {
    return resume;
  }

  // Build condensed version
  const parts: string[] = [];

  for (const section of analysis.sections) {
    if (section.priority >= 8) {
      // High priority: keep full or use summary
      const content = analysis.summaries[section.name] || section.content;
      parts.push(`${section.name.toUpperCase()}\n${content}`);
    } else if (section.priority >= 5) {
      // Medium priority: keep header only
      const lines = section.content.split('\n').filter((l) => l.trim());
      parts.push(`${section.name.toUpperCase()}\n${lines.slice(0, 2).join('\n')}\n[... truncated]`);
    }
    // Low priority sections are dropped
  }

  return parts.join('\n\n');
}

/** Summary statistics for a resume. */
export function getResumeStats(resume: string) {
  const tokens = estimateTokens(resume);
  const pages = estimatePages(resume);
  const sections = detectSections(resume);
  const chars = resume.length;
  // `.split(/\s+/)` yields `['']` for empty input and empty tokens for
  // leading/trailing whitespace, so trim first and bail out on empty.
  const trimmed = resume.trim();
  const words = trimmed ? trimmed.split(/\s+/).length : 0;

  return {
    characters: chars,
    words,
    tokens,
    estimatedPages: pages,
    sections: sections.length,
    sectionDetails: sections.map((s) => ({
      name: s.name,
      tokens: s.tokenCount,
      priority: s.priority,
    })),
    needsTruncation: tokens > 6000,
    strategy: tokens > 6000 ? 'multi-pass' : 'single-pass',
  };
}
