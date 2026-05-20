import { usePreferencesStore } from '@/store/preferences-store';

/**
 * Preferences Integration Service
 *
 * This service connects user preferences to actual application behavior:
 * - Scraping filters
 * - Ranking engine
 * - AI prompts
 * - Search behavior
 */

export interface ScrapingFilters {
  location?: string;
  remote?: 'remote' | 'hybrid' | 'on-site' | 'any';
  techStack?: string[];
  seniority?: string;
  salaryMin?: number;
  salaryMax?: number;
  salaryCurrency?: string;
}

export interface RankingWeights {
  locationMatch: number;
  techStackMatch: number;
  seniorityMatch: number;
  salaryMatch: number;
}

export interface AIPromptConfig {
  language: string;
  tone: 'professional' | 'casual' | 'formal' | 'creative';
  model?: string;
  temperature?: number;
  maxTokens?: number;
}

/**
 * Get scraping filters based on user preferences
 */
export function getScrapingFilters(): ScrapingFilters {
  const state = usePreferencesStore.getState();

  const filters: ScrapingFilters = {};

  if (state.location?.city) {
    filters.location = state.location.city;
  }

  if (state.remote && state.remote !== 'any') {
    filters.remote = state.remote;
  }

  if (state.techStack.length > 0) {
    filters.techStack = state.techStack.map((item) => item.name);
  }

  if (state.seniority && state.seniority !== 'any') {
    filters.seniority = state.seniority;
  }

  if (state.salary?.min) {
    filters.salaryMin = state.salary.min;
  }

  if (state.salary?.max) {
    filters.salaryMax = state.salary.max;
  }

  if (state.salary?.currency) {
    filters.salaryCurrency = state.salary.currency;
  }

  return filters;
}

/**
 * Get ranking weights based on user preferences
 * Higher weights mean those factors are more important to the user
 */
export function getRankingWeights(): RankingWeights {
  const state = usePreferencesStore.getState();

  const weights: RankingWeights = {
    locationMatch: 1.0,
    techStackMatch: 1.0,
    seniorityMatch: 1.0,
    salaryMatch: 1.0,
  };

  // If user has specific location preference, weight location matching higher
  if (state.location?.city) {
    weights.locationMatch = 1.5;
  }

  // If user has specific tech stack, weight tech stack matching higher
  if (state.techStack.length > 0) {
    weights.techStackMatch = 1.8;
  }

  // If user has specific seniority preference, weight seniority matching higher
  if (state.seniority && state.seniority !== 'any') {
    weights.seniorityMatch = 1.3;
  }

  // If user has salary expectations, weight salary matching higher
  if (state.salary?.min || state.salary?.max) {
    weights.salaryMatch = 1.2;
  }

  return weights;
}

/**
 * Get AI prompt configuration based on user preferences
 */
export function getAIPromptConfig(): AIPromptConfig {
  const state = usePreferencesStore.getState();

  const config: AIPromptConfig = {
    language: state.language,
    tone: state.outputTone,
  };

  if (state.aiModel?.defaultModel) {
    config.model = state.aiModel.defaultModel;
  }

  if (state.aiModel?.temperature !== undefined) {
    config.temperature = state.aiModel.temperature;
  }

  if (state.aiModel?.maxTokens) {
    config.maxTokens = state.aiModel.maxTokens;
  }

  return config;
}

/**
 * Build system prompt for AI based on user preferences
 */
export function buildSystemPrompt(basePrompt: string): string {
  const config = getAIPromptConfig();

  let prompt = basePrompt;

  // Add language instruction
  prompt += `\n\nLanguage: You must respond in ${config.language}.`;

  // Add tone instruction
  const toneInstructions: Record<typeof config.tone, string> = {
    professional: 'Use professional, business-appropriate language. Be concise and clear.',
    casual: 'Use conversational, friendly language. Be approachable and natural.',
    formal: 'Use formal, structured language. Be detailed and thorough.',
    creative: 'Use engaging, expressive language. Be creative and dynamic.',
  };

  prompt += `\n\nTone: ${toneInstructions[config.tone]}`;

  // Add model-specific instructions if applicable
  if (config.model) {
    prompt += `\n\nModel: Using ${config.model} for optimal performance.`;
  }

  return prompt;
}

/**
 * Check if a job matches user preferences
 */
interface JobLike {
  location?: string;
  remote?: string | boolean;
  skills?: string[];
  techStack?: string[];
  salary?: number;
  seniority?: string;
  [key: string]: unknown;
}

export function jobMatchesPreferences(job: JobLike): { matches: boolean; score: number } {
  const state = usePreferencesStore.getState();
  let score = 0;
  let maxScore = 0;

  // Location match
  maxScore += 1.5;
  if (state.location?.city && job.location) {
    if (job.location.toLowerCase().includes(state.location.city.toLowerCase())) {
      score += 1.5;
    }
  }

  // Remote match
  maxScore += 1.0;
  if (state.remote !== 'any' && job.remote) {
    if (job.remote === state.remote) {
      score += 1.0;
    }
  } else if (state.remote === 'any') {
    score += 1.0;
  }

  // Tech stack match
  maxScore += 1.8;
  if (state.techStack.length > 0 && job.techStack) {
    const jobTech = job.techStack.map((t: string) => t.toLowerCase());
    const userTech = state.techStack.map((t) => t.name.toLowerCase());
    const matches = jobTech.filter((t: string) => userTech.includes(t));
    score += (matches.length / userTech.length) * 1.8;
  }

  // Seniority match
  maxScore += 1.3;
  if (state.seniority && state.seniority !== 'any' && job.seniority) {
    if (job.seniority === state.seniority) {
      score += 1.3;
    }
  }

  // Salary match
  maxScore += 1.2;
  if (state.salary?.min || state.salary?.max) {
    if (job.salary) {
      if (state.salary.min && job.salary >= state.salary.min) {
        score += 0.6;
      }
      if (state.salary.max && job.salary <= state.salary.max) {
        score += 0.6;
      }
    }
  }

  const normalizedScore = maxScore > 0 ? score / maxScore : 1;

  return {
    matches: normalizedScore > 0.5,
    score: normalizedScore,
  };
}

/**
 * Subscribe to preference changes and trigger updates
 */
export function subscribeToPreferences(callback: () => void): () => void {
  return usePreferencesStore.subscribe(() => {
    callback();
  });
}
