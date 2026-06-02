/**
 * Provider-aware prompt layer.
 *
 * Prompts adapt to the three AI provider classes the app supports — local
 * **ollama** models, **cloud** API keys, and **cli** agents — choosing prompt
 * verbosity, truncation strategy, schema variant, and output-format. This module
 * stays PURE: it describes provider capabilities declaratively (flags + metadata);
 * the real API calls and CLI invocations live in the caller.
 */

import {
  detectModelSize,
  LARGE_MODEL_STRATEGY,
  MEDIUM_MODEL_STRATEGY,
  SMALL_MODEL_STRATEGY,
  type TruncationStrategy,
} from '../context-manager/index.js';

export type ProviderKind = 'ollama' | 'cloud' | 'cli';
export type PromptTier = 'large' | 'medium' | 'small';

export interface ProviderProfile {
  kind: ProviderKind;
  /** Raw model / tag name (e.g. `llama3.2:1b`, `gpt-4o`). */
  model?: string;
  /** Context window in tokens, if known — sizes cloud input budgets. */
  contextWindow?: number;
  /** Native JSON-schema / tool-use support (cloud). */
  supportsStructuredOutput?: boolean;
  /** Explicit ollama sub-tier; otherwise derived from `model`. */
  sizeHint?: PromptTier;
}

/**
 * What every builder accepts: either a legacy tier string (still works verbatim →
 * backward compatible) or a full {@link ProviderProfile}.
 */
export type PromptTarget = PromptTier | ProviderProfile;

/** Prompt framing depth. */
export type PromptDepth = 'brief' | 'full' | 'task';

export interface ResolvedProfile {
  kind: ProviderKind;
  tier: PromptTier;
  /** brief = compact/imperative, full = multi-perspective, task = agent task brief. */
  depth: PromptDepth;
  /** Output schema variant for analysis JSON. */
  schema: 'compact' | 'full';
  /** Whether to request the rich `rewrites` field. */
  includeRewrites: boolean;
  /** Whether the caller can request native structured output. */
  structuredOutput: boolean;
  /** Resume truncation strategy. */
  truncation: TruncationStrategy;
  /** Char cap for the resume slice. */
  resumeChars: number;
  /** Char cap for the job-ad slice. */
  jobAdChars: number;
}

function isProfile(t: PromptTarget): t is ProviderProfile {
  return typeof t === 'object' && t !== null && 'kind' in t;
}

function resolveTruncation(profile: ProviderProfile, tier: PromptTier): TruncationStrategy {
  if (profile.kind === 'cloud') {
    // Minimal truncation — size to the real context window, preserve all sections.
    const window = profile.contextWindow ?? 0;
    const maxTokens = window > 0 ? Math.min(Math.floor(window * 0.5), 24000) : 12000;
    return { ...LARGE_MODEL_STRATEGY, maxTokens, summarizeSections: [], dropSections: [] };
  }
  if (profile.kind === 'cli') {
    // Moderate — the agent can re-expand context itself if needed.
    return { ...LARGE_MODEL_STRATEGY, maxTokens: 8000 };
  }
  // ollama: aggressive, by sub-tier.
  if (tier === 'small') return SMALL_MODEL_STRATEGY;
  if (tier === 'medium') return MEDIUM_MODEL_STRATEGY;
  return LARGE_MODEL_STRATEGY;
}

/**
 * Normalize a {@link PromptTarget} into the concrete decisions a builder needs.
 *
 * - **cli** → `task` brief (self-verifying, full schema, moderate truncation).
 * - **cloud** → `full` multi-perspective prompt + rich schema + native structured
 *   output, minimal truncation sized to the context window.
 * - **ollama** → `full` only for a large local model, else `brief` (compact prompt,
 *   compact schema, aggressive truncation). Unknown local models default to the
 *   smaller/safer `brief` path via {@link detectModelSize}.
 *
 * A bare tier string is treated as a local model with that size — which finally
 * splits the old "medium == large" conflation (medium → brief).
 */
export function resolveProfile(target: PromptTarget = 'large'): ResolvedProfile {
  const profile: ProviderProfile = isProfile(target)
    ? target
    : { kind: 'ollama', sizeHint: target };

  const tier: PromptTier =
    profile.sizeHint ?? (profile.model ? detectModelSize(profile.model) : 'medium');

  const depth: PromptDepth =
    profile.kind === 'cli'
      ? 'task'
      : profile.kind === 'cloud'
        ? 'full'
        : tier === 'large'
          ? 'full'
          : 'brief';

  const schema: 'compact' | 'full' = depth === 'brief' ? 'compact' : 'full';
  const structuredOutput = profile.supportsStructuredOutput ?? profile.kind === 'cloud';

  return {
    kind: profile.kind,
    tier,
    depth,
    schema,
    includeRewrites: schema === 'full',
    structuredOutput,
    truncation: resolveTruncation(profile, tier),
    // resumeChars is retained for backward-compat / callers that still slice;
    // the generation builders now feed the résumé via `truncation` +
    // truncateResume() instead, which preserves whole high-value sections.
    resumeChars: depth === 'brief' ? 2500 : profile.kind === 'cloud' ? 8000 : 5000,
    // Job ads are not the candidate's data and often bury requirements late, so
    // give the extractor/generator more of the ad to read.
    jobAdChars: depth === 'brief' ? 2500 : profile.kind === 'cloud' ? 6000 : 5000,
  };
}

// ─── Structured-output metadata (for the caller) ──────────────────────────────

const SECTION_SCORE = {
  type: 'object',
  properties: { score: { type: 'number' }, feedback: { type: 'string' } },
  required: ['score', 'feedback'],
} as const;

const ENUM_PRIORITY = { type: 'string', enum: ['high', 'medium', 'low'] } as const;

/** JSON Schema for the analysis result — mirrors the `SCHEMA` in analyze/schema.ts. */
export const ANALYSIS_JSON_SCHEMA = {
  type: 'object',
  properties: {
    detectedLanguages: {
      type: 'object',
      properties: {
        resume: { type: 'string' },
        jobAd: { type: 'string' },
        mismatch: { type: 'boolean' },
      },
      required: ['resume', 'jobAd', 'mismatch'],
    },
    scores: {
      type: 'object',
      properties: {
        ats: { type: 'number' },
        jobMatch: { type: 'number' },
        languageAlignment: { type: 'number' },
        readability: { type: 'number' },
        keywordCoverage: { type: 'number' },
      },
      required: ['ats', 'jobMatch', 'languageAlignment', 'readability', 'keywordCoverage'],
    },
    summary: {
      type: 'object',
      properties: {
        strengths: { type: 'array', items: { type: 'string' } },
        weaknesses: { type: 'array', items: { type: 'string' } },
        overallAssessment: { type: 'string' },
      },
      required: ['strengths', 'weaknesses', 'overallAssessment'],
    },
    missingKeywords: { type: 'array', items: { type: 'string' } },
    matchedSkills: { type: 'array', items: { type: 'string' } },
    missingSkills: { type: 'array', items: { type: 'string' } },
    recommendations: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          priority: ENUM_PRIORITY,
          text: { type: 'string' },
          category: {
            type: 'string',
            enum: ['keyword', 'skill', 'format', 'language', 'experience'],
          },
        },
        required: ['priority', 'text', 'category'],
      },
    },
    sectionAnalysis: {
      type: 'object',
      properties: {
        summary: SECTION_SCORE,
        experience: SECTION_SCORE,
        skills: SECTION_SCORE,
        education: SECTION_SCORE,
        formatting: SECTION_SCORE,
      },
      required: ['summary', 'experience', 'skills', 'education', 'formatting'],
    },
    rewrites: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          section: { type: 'string' },
          original: { type: 'string' },
          improved: { type: 'string' },
          reason: { type: 'string' },
        },
        required: ['section', 'original', 'improved', 'reason'],
      },
    },
    languageRecommendations: { type: 'array', items: { type: 'string' } },
    atsRisks: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          severity: ENUM_PRIORITY,
          issue: { type: 'string' },
          fix: { type: 'string' },
        },
        required: ['severity', 'issue', 'fix'],
      },
    },
    recruiterPerspective: { type: 'string' },
    finalVerdict: { type: 'string' },
  },
  required: [
    'detectedLanguages',
    'scores',
    'summary',
    'missingKeywords',
    'matchedSkills',
    'missingSkills',
    'recommendations',
    'sectionAnalysis',
    'recruiterPerspective',
    'finalVerdict',
  ],
} as const;

/** JSON Schema for the generation metadata extraction. */
export const METADATA_JSON_SCHEMA = {
  type: 'object',
  properties: {
    candidateName: { type: 'string' },
    jobTitle: { type: 'string' },
    companyName: { type: 'string' },
    resumeLanguage: { type: 'string' },
    jobAdLanguage: { type: 'string' },
    topRequirements: { type: 'array', items: { type: 'string' } },
    candidateSeniority: { type: 'string', enum: ['junior', 'mid', 'senior', 'lead', 'executive'] },
    jobLocation: { type: 'string' },
    jobCountry: { type: 'string' },
  },
  required: [
    'candidateName',
    'jobTitle',
    'companyName',
    'resumeLanguage',
    'jobAdLanguage',
    'topRequirements',
  ],
} as const;

export interface StructuredOutputSpec {
  type: 'json_schema';
  name: string;
  schema: object;
}

/**
 * The native structured-output request the caller should attach (OpenAI
 * `response_format`, Anthropic forced tool-use, Gemini `responseSchema`), or
 * `null` when the target can't do native structured output (ollama/cli) — in
 * which case the caller relies on `validateAndRepair` / `validateMetadata`.
 */
export function structuredOutputFor(
  target: PromptTarget,
  kind: 'analysis' | 'metadata'
): StructuredOutputSpec | null {
  if (!resolveProfile(target).structuredOutput) return null;
  return kind === 'analysis'
    ? { type: 'json_schema', name: 'resume_analysis', schema: ANALYSIS_JSON_SCHEMA }
    : { type: 'json_schema', name: 'generation_metadata', schema: METADATA_JSON_SCHEMA };
}
