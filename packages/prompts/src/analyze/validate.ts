/** Output validator + JSON repair for the analysis result. */

import type { AnalysisResult, AnalysisScore } from './schema.js';

function extractJSON(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.startsWith('{')) return trimmed;
  // No `\s*` before the lazy capture — it overlapped the `[\s\S]*?` and made the
  // match polynomial (js/polynomial-redos). The group is `.trim()`-ed below, so
  // dropping it preserves the extracted JSON exactly.
  const fenced = trimmed.match(/```(?:json)?([\s\S]*?)```/);
  if (fenced?.[1]) return fenced[1].trim();
  const start = trimmed.indexOf('{');
  const end = trimmed.lastIndexOf('}');
  if (start !== -1 && end > start) return trimmed.slice(start, end + 1);
  return null;
}

/**
 * Coerce a model-provided score to a clamped 0–100 integer, or `null` when the
 * value is missing/garbled. Returning `null` (instead of a confident 50) keeps
 * the result honest — the UI renders an explicit "Not scored" state rather than
 * a fabricated midpoint.
 */
function clampScore(v: unknown, min = 0, max = 100): AnalysisScore {
  if (v === null || v === undefined) return null;
  // Blank / whitespace-only strings are "not scored" — without the trim, a
  // value like `' '` would slip past the `=== ''` check and coerce to 0.
  if (typeof v === 'string' && v.trim().length === 0) return null;
  const n = typeof v === 'number' ? v : Number(v);
  if (!Number.isFinite(n)) return null;
  return Math.max(min, Math.min(max, Math.round(n)));
}

function ensureArray(v: unknown): unknown[] {
  return Array.isArray(v) ? v : [];
}

function ensureString(v: unknown, fallback = ''): string {
  return typeof v === 'string' && v.length > 0 ? v : fallback;
}

export function validateAndRepair(raw: string): AnalysisResult | null {
  const jsonStr = extractJSON(raw);
  if (!jsonStr) return null;

  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(jsonStr);
  } catch {
    try {
      const cleaned = jsonStr.replace(/,\s*([}\]])/g, '$1');
      parsed = JSON.parse(cleaned);
    } catch {
      return null;
    }
  }

  const p = parsed as Record<string, unknown>;
  const langs = (p.detectedLanguages as Record<string, unknown>) ?? {};
  const scores = (p.scores as Record<string, unknown>) ?? {};
  const summary = (p.summary as Record<string, unknown>) ?? {};
  const secAn = (p.sectionAnalysis as Record<string, unknown>) ?? {};

  const sectionFor = (key: string): AnalysisResult['sectionAnalysis']['summary'] => {
    const s = (secAn[key] as Record<string, unknown>) ?? {};
    return {
      score: clampScore(s.score),
      feedback: ensureString(s.feedback, 'No feedback provided.'),
    };
  };

  const recs = ensureArray(p.recommendations)
    .map((r) => {
      const ro = (r as Record<string, unknown>) ?? {};
      return {
        priority: (['high', 'medium', 'low'].includes(ro.priority as string)
          ? ro.priority
          : 'medium') as 'high' | 'medium' | 'low',
        text: ensureString(ro.text),
        category: (['keyword', 'skill', 'format', 'language', 'experience'].includes(
          ro.category as string
        )
          ? ro.category
          : 'skill') as AnalysisResult['recommendations'][number]['category'],
      };
    })
    .filter((r) => r.text.length > 0);

  const rewrites = ensureArray(p.rewrites)
    .map((r) => {
      const ro = (r as Record<string, unknown>) ?? {};
      return {
        section: ensureString(ro.section),
        original: ensureString(ro.original),
        improved: ensureString(ro.improved),
        reason: ensureString(ro.reason),
      };
    })
    .filter((r) => r.improved.length > 0);

  const risks = ensureArray(p.atsRisks)
    .map((r) => {
      const ro = (r as Record<string, unknown>) ?? {};
      return {
        severity: (['high', 'medium', 'low'].includes(ro.severity as string)
          ? ro.severity
          : 'medium') as 'high' | 'medium' | 'low',
        issue: ensureString(ro.issue),
        fix: ensureString(ro.fix),
      };
    })
    .filter((r) => r.issue.length > 0);

  // Coerce first, then decide mismatch on the coerced values — an empty-string
  // language from the model would otherwise slip past the 'unknown' guard below.
  const resumeLang = ensureString(langs.resume, 'unknown');
  const jobAdLang = ensureString(langs.jobAd, 'unknown');
  return {
    detectedLanguages: {
      resume: resumeLang,
      jobAd: jobAdLang,
      // Only flag mismatch when both languages are actually known — matches
      // detectLanguages() in @ajh/shared. Otherwise an 'unknown' or missing
      // side would falsely trip the mismatch warning.
      mismatch:
        langs.mismatch === true ||
        (resumeLang !== 'unknown' && jobAdLang !== 'unknown' && resumeLang !== jobAdLang),
    },
    scores: {
      ats: clampScore(scores.ats),
      jobMatch: clampScore(scores.jobMatch),
      languageAlignment: clampScore(scores.languageAlignment),
      readability: clampScore(scores.readability),
      keywordCoverage: clampScore(scores.keywordCoverage),
    },
    summary: {
      strengths: ensureArray(summary.strengths)
        .map((s) => ensureString(s))
        .filter(Boolean),
      weaknesses: ensureArray(summary.weaknesses)
        .map((s) => ensureString(s))
        .filter(Boolean),
      overallAssessment: ensureString(summary.overallAssessment),
    },
    missingKeywords: ensureArray(p.missingKeywords)
      .map((s) => ensureString(s))
      .filter(Boolean),
    matchedSkills: ensureArray(p.matchedSkills)
      .map((s) => ensureString(s))
      .filter(Boolean),
    missingSkills: ensureArray(p.missingSkills)
      .map((s) => ensureString(s))
      .filter(Boolean),
    recommendations: recs,
    sectionAnalysis: {
      summary: sectionFor('summary'),
      experience: sectionFor('experience'),
      skills: sectionFor('skills'),
      education: sectionFor('education'),
      formatting: sectionFor('formatting'),
    },
    rewrites,
    languageRecommendations: ensureArray(p.languageRecommendations)
      .map((s) => ensureString(s))
      .filter(Boolean),
    atsRisks: risks,
    recruiterPerspective: ensureString(p.recruiterPerspective),
    finalVerdict: ensureString(p.finalVerdict),
  };
}
