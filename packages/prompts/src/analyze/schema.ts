/** Resume-analysis result type + JSON schema strings (compact / full). */

/**
 * A score the model was asked to produce. `null` is the explicit "not scored"
 * sentinel — the model omitted or garbled the value, so we must NOT invent a
 * confident midpoint (the old behaviour silently returned 50). Valid values are
 * clamped to 0–100.
 */
export type AnalysisScore = number | null;

export interface AnalysisResult {
  detectedLanguages: {
    resume: string;
    jobAd: string;
    mismatch: boolean;
  };
  scores: {
    ats: AnalysisScore; // 0-100 or null when not scored
    jobMatch: AnalysisScore; // 0-100 or null when not scored
    languageAlignment: AnalysisScore; // 0-100 or null when not scored
    readability: AnalysisScore; // 0-100 or null when not scored
    keywordCoverage: AnalysisScore; // 0-100 or null when not scored
  };
  summary: {
    strengths: string[];
    weaknesses: string[];
    overallAssessment: string;
  };
  missingKeywords: string[];
  matchedSkills: string[];
  missingSkills: string[];
  recommendations: Array<{
    priority: 'high' | 'medium' | 'low';
    text: string;
    category: 'keyword' | 'skill' | 'format' | 'language' | 'experience';
  }>;
  sectionAnalysis: {
    summary: { score: AnalysisScore; feedback: string };
    experience: { score: AnalysisScore; feedback: string };
    skills: { score: AnalysisScore; feedback: string };
    education: { score: AnalysisScore; feedback: string };
    formatting: { score: AnalysisScore; feedback: string };
  };
  rewrites: Array<{
    section: string;
    original: string;
    improved: string;
    reason: string;
  }>;
  languageRecommendations: string[];
  atsRisks: Array<{
    severity: 'high' | 'medium' | 'low';
    issue: string;
    fix: string;
  }>;
  recruiterPerspective: string;
  finalVerdict: string;
}

// Compact schema for small local models — omits rewrites (hallucination-prone),
// caps list lengths, keeps full sectionAnalysis structure so the UI still works.
export const SCHEMA_COMPACT = `{
  "detectedLanguages": { "resume": "string", "jobAd": "string", "mismatch": boolean },
  "scores": { "ats": 0-100, "jobMatch": 0-100, "languageAlignment": 0-100, "readability": 0-100, "keywordCoverage": 0-100 },
  "summary": { "strengths": ["string (max 3)"], "weaknesses": ["string (max 3)"], "overallAssessment": "string" },
  "missingKeywords": ["string (max 8)"],
  "matchedSkills": ["string (max 8)"],
  "missingSkills": ["string (max 5)"],
  "recommendations": [{ "priority": "high|medium|low", "text": "string", "category": "keyword|skill|format|language|experience" }],
  "sectionAnalysis": {
    "summary":    { "score": 0-100, "feedback": "string" },
    "experience": { "score": 0-100, "feedback": "string" },
    "skills":     { "score": 0-100, "feedback": "string" },
    "education":  { "score": 0-100, "feedback": "string" },
    "formatting": { "score": 0-100, "feedback": "string" }
  },
  "rewrites": [],
  "languageRecommendations": ["string (max 2)"],
  "atsRisks": [{ "severity": "high|medium|low", "issue": "string", "fix": "string" }],
  "recruiterPerspective": "string",
  "finalVerdict": "string"
}`;

export const SCHEMA = `{
  "detectedLanguages": { "resume": "string", "jobAd": "string", "mismatch": boolean },
  "scores": { "ats": 0-100, "jobMatch": 0-100, "languageAlignment": 0-100, "readability": 0-100, "keywordCoverage": 0-100 },
  "summary": { "strengths": ["string"], "weaknesses": ["string"], "overallAssessment": "string" },
  "missingKeywords": ["string"],
  "matchedSkills": ["string"],
  "missingSkills": ["string"],
  "recommendations": [{ "priority": "high|medium|low", "text": "string", "category": "keyword|skill|format|language|experience" }],
  "sectionAnalysis": {
    "summary":    { "score": 0-100, "feedback": "string" },
    "experience": { "score": 0-100, "feedback": "string" },
    "skills":     { "score": 0-100, "feedback": "string" },
    "education":  { "score": 0-100, "feedback": "string" },
    "formatting": { "score": 0-100, "feedback": "string" }
  },
  "rewrites": [{ "section": "string", "original": "string", "improved": "string", "reason": "string" }],
  "languageRecommendations": ["string"],
  "atsRisks": [{ "severity": "high|medium|low", "issue": "string", "fix": "string" }],
  "recruiterPerspective": "string",
  "finalVerdict": "string"
}`;
