/** Locale-aware resume section detection. */

import { SECTION_LEXICON } from '../locale/index.js';
import { estimateTokens } from './tokens.js';

export interface ResumeSection {
  name: string;
  content: string;
  startIndex: number;
  endIndex: number;
  priority: number; // 1-10, higher = more important
  tokenCount: number;
}

/**
 * Detect resume sections using locale-aware header lexicons (en, de, fr, es, it,
 * nl, pt). Detection matches against the combined lexicon, so a resume in a
 * different language than the UI still segments correctly instead of collapsing
 * into one blob. `locale` is used for per-language token estimation.
 */
export function detectSections(resume: string, locale?: string): ResumeSection[] {
  const sections: ResumeSection[] = [];
  const lines = resume.split('\n');
  let current: ResumeSection | null = null;
  let content: string[] = [];

  const finalize = (endIndex: number) => {
    if (!current) return;
    current.content = content.join('\n').trim();
    current.endIndex = endIndex;
    current.tokenCount = estimateTokens(current.content, locale);
    sections.push(current);
  };

  for (let i = 0; i < lines.length; i++) {
    const line = (lines[i] ?? '').trim();
    const header = line ? detectHeader(line) : null;

    if (header) {
      finalize(i - 1);
      current = {
        name: header.name,
        content: '',
        startIndex: i,
        endIndex: i,
        priority: header.priority,
        tokenCount: 0,
      };
      content = [];
    } else if (current) {
      content.push(line);
    } else {
      // Content before the first detected header (usually contact info).
      current = {
        name: 'Header',
        content: '',
        startIndex: i,
        endIndex: i,
        priority: 10,
        tokenCount: 0,
      };
      content = [line];
    }
  }

  finalize(lines.length - 1);
  return sections;
}

/** Whether `lowerLine` begins with one of `terms` as a whole header word. */
function matchesHeaderTerm(lowerLine: string, terms: string[]): boolean {
  for (const term of terms) {
    if (lowerLine === term) return true;
    if (lowerLine.startsWith(term)) {
      const next = lowerLine.charAt(term.length);
      if (next === ' ' || next === ':' || next === '\t' || next === '|' || next === '-')
        return true;
    }
  }
  return false;
}

/** Classify a line as a section header via the multi-locale lexicon. */
function detectHeader(line: string): { name: string; priority: number } | null {
  const lower = line.toLowerCase();
  for (const { name, priority, terms } of SECTION_LEXICON) {
    if (matchesHeaderTerm(lower, terms)) return { name, priority };
  }
  return null;
}
