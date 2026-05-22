/**
 * Context Management for Large Resumes
 *
 * Handles resumes that exceed token limits (5+ pages, 10k+ words).
 * Uses intelligent chunking, summarization, and priority-based context selection.
 */

// ─── Token Estimation ─────────────────────────────────────────────────────────

/**
 * Estimate token count (rough approximation: 1 token ≈ 4 characters)
 * More accurate than word count for LLM context limits.
 */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

/**
 * Estimate page count based on character count
 * Average page: ~3000 characters (500 words × 6 chars/word)
 */
export function estimatePages(text: string): number {
  return Math.ceil(text.length / 3000);
}

// ─── Resume Structure Detection ───────────────────────────────────────────────

export interface ResumeSection {
  name: string;
  content: string;
  startIndex: number;
  endIndex: number;
  priority: number; // 1-10, higher = more important
  tokenCount: number;
}

/**
 * Detect resume sections using common headers
 */
export function detectSections(resume: string): ResumeSection[] {
  const sections: ResumeSection[] = [];

  // Common section headers (case-insensitive, with variations)
  const sectionPatterns = [
    {
      pattern: /^(professional\s+summary|summary|profile|objective|about\s+me)/im,
      name: 'Summary',
      priority: 9,
    },
    {
      pattern:
        /^(work\s+experience|experience|employment\s+history|career|professional\s+experience)/im,
      name: 'Experience',
      priority: 10,
    },
    {
      pattern: /^(education|academic\s+background|qualifications)/im,
      name: 'Education',
      priority: 8,
    },
    {
      pattern: /^(skills|technical\s+skills|core\s+competencies|expertise)/im,
      name: 'Skills',
      priority: 9,
    },
    { pattern: /^(certifications|certificates|licenses)/im, name: 'Certifications', priority: 7 },
    { pattern: /^(projects|portfolio|key\s+projects)/im, name: 'Projects', priority: 6 },
    { pattern: /^(publications|research|papers)/im, name: 'Publications', priority: 5 },
    { pattern: /^(awards|honors|achievements)/im, name: 'Awards', priority: 5 },
    { pattern: /^(languages|language\s+skills)/im, name: 'Languages', priority: 4 },
    { pattern: /^(volunteer|volunteering|community)/im, name: 'Volunteer', priority: 3 },
    { pattern: /^(interests|hobbies)/im, name: 'Interests', priority: 2 },
  ];

  const lines = resume.split('\n');
  let currentSection: ResumeSection | null = null;
  let currentContent: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // Check if this line is a section header
    let foundSection = false;
    for (const { pattern, name, priority } of sectionPatterns) {
      if (pattern.test(line)) {
        // Save previous section
        if (currentSection) {
          currentSection.content = currentContent.join('\n').trim();
          currentSection.endIndex = i - 1;
          currentSection.tokenCount = estimateTokens(currentSection.content);
          sections.push(currentSection);
        }

        // Start new section
        currentSection = {
          name,
          content: '',
          startIndex: i,
          endIndex: i,
          priority,
          tokenCount: 0,
        };
        currentContent = [];
        foundSection = true;
        break;
      }
    }

    if (!foundSection && currentSection) {
      currentContent.push(line);
    } else if (!foundSection && !currentSection) {
      // Content before first section (usually contact info)
      if (!sections.length) {
        currentSection = {
          name: 'Header',
          content: '',
          startIndex: 0,
          endIndex: 0,
          priority: 10,
          tokenCount: 0,
        };
        currentContent = [line];
      }
    }
  }

  // Save last section
  if (currentSection) {
    currentSection.content = currentContent.join('\n').trim();
    currentSection.endIndex = lines.length - 1;
    currentSection.tokenCount = estimateTokens(currentSection.content);
    sections.push(currentSection);
  }

  return sections;
}

// ─── Smart Truncation Strategies ──────────────────────────────────────────────

export interface TruncationStrategy {
  maxTokens: number;
  preserveSections: string[]; // Section names to always keep
  summarizeSections: string[]; // Section names to summarize if too long
  dropSections: string[]; // Section names to drop if needed
  modelType?: 'large' | 'medium' | 'small'; // Model size hint
}

/**
 * Strategy for large models (GPT-4, Claude, Gemini)
 * Context window: 8K-128K tokens
 */
export const LARGE_MODEL_STRATEGY: TruncationStrategy = {
  maxTokens: 6000, // Conservative limit
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills', 'Education'],
  summarizeSections: ['Projects', 'Certifications'],
  dropSections: ['Interests', 'Volunteer', 'Languages', 'Awards', 'Publications'],
  modelType: 'large',
};

/**
 * Strategy for medium models (Llama 3 8B, Mistral 7B)
 * Context window: 4K-8K tokens
 */
export const MEDIUM_MODEL_STRATEGY: TruncationStrategy = {
  maxTokens: 3500, // Leave room for job ad + prompt
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills'],
  summarizeSections: ['Education', 'Certifications'],
  dropSections: ['Projects', 'Interests', 'Volunteer', 'Languages', 'Awards', 'Publications'],
  modelType: 'medium',
};

/**
 * Strategy for small local models (Llama 3.2 1B-3B, Phi-3, Gemma 2B)
 * Context window: 2K-4K tokens
 */
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

/**
 * Default strategy for resume analysis (alias for large model)
 */
export const ANALYSIS_STRATEGY = LARGE_MODEL_STRATEGY;

/**
 * Strategy for resume generation (needs more context)
 */
export const GENERATION_STRATEGY: TruncationStrategy = {
  maxTokens: 5000,
  preserveSections: ['Header', 'Summary', 'Experience', 'Skills', 'Education'],
  summarizeSections: ['Projects', 'Certifications', 'Awards'],
  dropSections: ['Interests', 'Volunteer', 'Publications'],
  modelType: 'large',
};

/**
 * Truncate experience section intelligently
 * Keep most recent roles, summarize older ones
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
  let result: string[] = [];
  let currentTokens = 0;

  for (let i = 0; i < roles.length; i++) {
    const role = roles[i];
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
        currentTokens += headerTokens + 10;
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

/**
 * Summarize a section to reduce token count
 */
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

/**
 * Detect model size from model name
 */
export function detectModelSize(modelName: string): 'large' | 'medium' | 'small' {
  const name = modelName.toLowerCase();

  // Large models (8K+ context)
  if (
    name.includes('gpt-4') ||
    name.includes('claude') ||
    name.includes('gemini-pro') ||
    name.includes('gemini-2.0') ||
    name.includes('command-r')
  ) {
    return 'large';
  }

  // Small models (2K-4K context)
  if (
    name.includes('llama3.2:1b') ||
    name.includes('llama3.2:3b') ||
    name.includes('phi-3') ||
    name.includes('gemma:2b') ||
    name.includes('gemma:7b') ||
    name.includes('qwen2:0.5b') ||
    name.includes('qwen2:1.5b') ||
    name.includes('tinyllama') ||
    name.includes('stablelm')
  ) {
    return 'small';
  }

  // Medium models (4K-8K context) - default for most local models
  return 'medium';
}

/**
 * Get appropriate strategy based on model name
 */
export function getStrategyForModel(modelName: string): TruncationStrategy {
  const size = detectModelSize(modelName);

  switch (size) {
    case 'large':
      return LARGE_MODEL_STRATEGY;
    case 'medium':
      return MEDIUM_MODEL_STRATEGY;
    case 'small':
      return SMALL_MODEL_STRATEGY;
    default:
      return MEDIUM_MODEL_STRATEGY;
  }
}

/**
 * Intelligently truncate resume to fit within token limit
 */
export function truncateResume(resume: string, strategy: TruncationStrategy): string {
  const sections = detectSections(resume);
  const totalTokens = sections.reduce((sum, s) => sum + s.tokenCount, 0);

  // If already under limit, return as-is
  if (totalTokens <= strategy.maxTokens) {
    return resume;
  }

  const modelType = strategy.modelType || 'large';
  console.log(
    `Resume too large: ${totalTokens} tokens (limit: ${strategy.maxTokens} for ${modelType} model)`
  );
  console.log(`Detected ${sections.length} sections, ${estimatePages(resume)} estimated pages`);

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
        // Section is too large, need to truncate it
        const available = strategy.maxTokens - currentTokens;
        if (available > 100) {
          const truncated =
            section.name === 'Experience'
              ? truncateExperience(section.content, available)
              : section.content.slice(0, available * 4); // Rough char estimate
          result.push(`${section.name.toUpperCase()}\n${truncated}`);
          currentTokens += estimateTokens(truncated);
        }
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
  console.log(
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

// ─── Multi-Pass Analysis for Very Large Resumes ───────────────────────────────

export interface MultiPassResult {
  sections: ResumeSection[];
  summaries: Record<string, string>;
  totalTokens: number;
  strategy: 'single-pass' | 'multi-pass';
}

/**
 * Analyze resume structure and determine if multi-pass is needed
 */
export function analyzeResumeSize(resume: string, maxTokens = 6000): MultiPassResult {
  const sections = detectSections(resume);
  const totalTokens = sections.reduce((sum, s) => sum + s.tokenCount, 0);

  if (totalTokens <= maxTokens) {
    return {
      sections,
      summaries: {},
      totalTokens,
      strategy: 'single-pass',
    };
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

  return {
    sections,
    summaries,
    totalTokens,
    strategy: 'multi-pass',
  };
}

/**
 * Create a condensed version of resume for initial analysis
 */
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

// ─── Export utilities ──────────────────────────────────────────────────────────

export function getResumeStats(resume: string) {
  const tokens = estimateTokens(resume);
  const pages = estimatePages(resume);
  const sections = detectSections(resume);
  const chars = resume.length;
  const words = resume.split(/\s+/).length;

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
