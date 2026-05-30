/** Resume-analysis result type + JSON schema strings (compact / full). */

export interface AnalysisResult {
  detectedLanguages: {
    resume: string;
    jobAd: string;
    mismatch: boolean;
  };
  scores: {
    ats: number; // 0-100
    jobMatch: number; // 0-100
    languageAlignment: number; // 0-100
    readability: number; // 0-100
    keywordCoverage: number; // 0-100
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
    summary: { score: number; feedback: string };
    experience: { score: number; feedback: string };
    skills: { score: number; feedback: string };
    education: { score: number; feedback: string };
    formatting: { score: number; feedback: string };
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
