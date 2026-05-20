/**
 * Resume Matching Engine — hybrid, explainable, multilingual.
 *
 *   Resume → Skill Extraction → Job Requirement Extraction →
 *   Semantic Matching → Weighted Ranking → AI Explanation
 */
import type { MatchScore } from '@ajh/shared';

export interface MatchInputs {
  resumeText: string;
  resumeSkills: string[];
  jobText: string;
  jobRequirements: string[];
  semanticSimilarity: number; // 0..1 — precomputed via vector dot/cosine
  locale?: string; // Application locale for recommendations
}

export interface MatchOptions {
  semanticWeight?: number; // 0..1 (default 0.6)
}

export class MatchingEngine {
  /** ATS-style keyword overlap score (0..1). */
  atsScore(resumeSkills: string[], jobRequirements: string[]): number {
    if (jobRequirements.length === 0) return 0;
    const set = new Set(resumeSkills.map(normalize));
    let hits = 0;
    for (const req of jobRequirements) if (set.has(normalize(req))) hits++;
    return hits / jobRequirements.length;
  }

  /** Identify requirements not present in the resume. */
  gaps(resumeSkills: string[], jobRequirements: string[]): string[] {
    const set = new Set(resumeSkills.map(normalize));
    return jobRequirements.filter((req) => !set.has(normalize(req)));
  }

  /** Weighted hybrid score. */
  combine(ats: number, semantic: number, opts: MatchOptions = {}): number {
    const w = clamp01(opts.semanticWeight ?? 0.6);
    return clamp01(ats * (1 - w) + semantic * w);
  }

  /** Produce a structured (not yet AI-explained) score. The AI explanation
   *  is layered in by the caller via the AI runtime once the numbers exist. */
  evaluate(
    resumeId: string,
    jobId: string,
    inputs: MatchInputs,
    opts: MatchOptions = {}
  ): Omit<MatchScore, 'explanation'> {
    const ats = this.atsScore(inputs.resumeSkills, inputs.jobRequirements);
    const semantic = clamp01(inputs.semanticSimilarity);
    const combined = this.combine(ats, semantic, opts);
    const gaps = this.gaps(inputs.resumeSkills, inputs.jobRequirements);
    // Recommendations will be generated via AI in the correct locale by the caller
    const recommendations: string[] = [];
    return { resumeId, jobId, ats, semantic, combined, gaps, recommendations };
  }
}

function normalize(s: string): string {
  return s
    .toLowerCase()
    .trim()
    .replace(/\s+/g, ' ')
    .replace(/[^\p{L}\p{N}+#.\- ]/gu, '');
}
function clamp01(n: number): number {
  return Math.max(0, Math.min(1, n));
}
