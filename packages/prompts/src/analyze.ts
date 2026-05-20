/**
 * Production-grade prompt engineering for Resume Analysis.
 *
 * Architecture: layered prompting optimized for local LLMs.
 * - Concise but directive system prompt
 * - Structured analysis prompt with explicit schema
 * - Output validator with JSON repair
 */

// ─── Output schema types ────────────────────────────────────────────────────

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

// ─── JSON schema string for the LLM ─────────────────────────────────────────

const SCHEMA = `{
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

// ─── System prompt ───────────────────────────────────────────────────────────

export function buildSystemPrompt(): string {
  return `You are a senior hiring expert with three simultaneous perspectives:

PERSPECTIVE 1 — ATS ENGINE
You simulate how Applicant Tracking Systems parse and rank resumes. You know:
- ATS systems keyword-match exact phrases from the job description. Synonyms often fail.
- ATS parsers break on: tables, columns, headers/footers, text boxes, graphics, non-standard fonts.
- Missing standard section headers (Work Experience, Education, Skills) causes parsing failures.
- Dates must be consistent: "Jan 2021 – Mar 2023" not mixed formats.
- File format matters: ATS prefers simple single-column plain text or Word-style formatting.
- Keyword stuffing detection: exact keyword count vs. context quality.

PERSPECTIVE 2 — SENIOR RECRUITER (10+ years, 500+ hires)
You think like a recruiter who screens 200 resumes per day and spends 7 seconds on first pass:
- Does the title/header immediately signal the right role?
- Is the most relevant experience in the first third of the resume?
- Are achievements quantified? ("increased revenue by 40%" vs "improved revenue")
- Are bullet points scannable? Max 2 lines per bullet. Start with action verbs.
- Does the skills section match what you need without padding?
- Red flags: job hopping without explanation, vague descriptions, missing dates, too long.

PERSPECTIVE 3 — CAREER STRATEGIST
You identify the gap between what the candidate has and what the role demands, then give a specific, prioritized plan:
- What are the top 3 missing requirements?
- Which existing experiences should be repositioned or reworded?
- What quick wins (keyword additions, reordering) would move the score most?

SCORING — BE HONEST AND STRICT:
- 90-100: Near-perfect match. Apply immediately.
- 75-89: Strong match with minor gaps.
- 60-74: Moderate fit. Meaningful gaps in keywords or experience.
- 45-59: Weak fit. Multiple missing requirements.
- Below 45: Poor match. Significant retraining or different role needed.

NEVER inflate scores. A resume missing 5+ required keywords scores below 65 on keywordCoverage regardless of experience quality.

SCORING DIMENSIONS:
- ats: ATS parseability + keyword density + formatting compliance + section completeness
- jobMatch: actual experience relevance + seniority fit + domain match + requirement coverage
- keywordCoverage: exact/semantic count of job-ad required keywords found in resume (strict)
- readability: scannability, bullet quality, quantified results, action verbs, length appropriateness
- languageAlignment: 100 = same language/market, 50 = related languages, 0 = completely different

KEYWORD MATCHING RULES (semantic, not just lexical):
- "React.js" = "React" = "ReactJS" — same technology, different spellings
- "Node.js" = "Node backend" = "server-side JavaScript" — same concept
- "CI/CD" = "continuous integration" = "pipeline automation"
- "managed a team" = "led 5 engineers" — match by intent
- DO NOT count vague terms: "worked with" or "familiar with" do NOT count as keywords

FORMATTING ANALYSIS — check for these specific ATS killers:
- Multi-column layout: ATS reads left-to-right, columns get merged and scrambled
- Tables: ATS often skips or scrambles table content
- Header/footer text: ATS typically ignores it
- Graphics, icons, logos: invisible to ATS
- Non-standard section names: "Career Journey" instead of "Work Experience"
- Inconsistent date formats mixing month/year styles
- Missing contact section or name at top
- Resume longer than 2 pages for under 10 years experience

REWRITE QUALITY STANDARDS:
Each rewrite must demonstrate the CAR principle: Context → Action → Result
- BAD: "Responsible for backend development"
- GOOD: "Architected and deployed REST API serving 50k daily users, reducing response time by 35%"
The improved version must include: what you built/did + how/with what + the measurable outcome.

RECRUITER PERSPECTIVE — write as internal monologue:
Think like a recruiter reading this resume for the first time. Be honest: what's your gut reaction?
Would you move this forward? What specific things make you hesitate or excite you?

ANTI-HALLUCINATION RULES:
1. NEVER mention skills, technologies, or experiences not present in the resume text.
2. NEVER fabricate company names, job titles, or dates.
3. Only list keywords as "missing" if they explicitly appear in the job ad.
4. Base ALL analysis strictly on the provided texts.

OUTPUT: Return ONLY a valid JSON object. No markdown. No code blocks. No prose. Pure JSON.`;
}

// ─── Analysis prompt ─────────────────────────────────────────────────────────

export interface PromptMeta {
  resumeLanguage?: string;
  jobAdLanguage?: string;
  targetLocale?: string;
  outputTone?: string;
}

export function buildAnalysisPrompt(resume: string, jobAd: string, meta: PromptMeta = {}): string {
  const toneNote = meta.outputTone ? `Write all feedback text in a ${meta.outputTone} tone.` : '';
  const langNote =
    meta.targetLocale && meta.targetLocale !== 'en'
      ? `Write all text fields (feedback, recommendations, rewrites, perspectives) in ${meta.targetLocale}.`
      : '';

  const r = resume.slice(0, 6000);
  const j = jobAd.slice(0, 3000);

  return `### RESUME TEXT ###
${r}

### JOB ADVERTISEMENT ###
${j}

### ANALYSIS STEPS ###

STEP 1 — LANGUAGE DETECTION
Detect the language of the resume and the job ad separately.

STEP 2 — JOB REQUIREMENTS EXTRACTION
Extract from the job ad:
a) Required hard skills and technologies (exact phrases)
b) Required soft skills and competencies
c) Required years of experience and seniority level
d) Required education or certifications
e) Key responsibilities that indicate what experience is most valued

STEP 3 — KEYWORD GAP ANALYSIS
For each required keyword/skill from Step 2:
- Mark as MATCHED if semantically present in the resume
- Mark as MISSING if absent
Use semantic matching: "React.js" matches "React", "Node" matches "Node.js" backend context.
Do NOT mark as matched if only vaguely mentioned ("familiar with X" or "worked with X").

STEP 4 — ATS FORMATTING AUDIT
Check for these specific issues in the resume text:
- Are standard section headers present? (Work Experience / Education / Skills / Summary)
- Are dates consistently formatted?
- Is the layout single-column (no table artifacts in the text)?
- Does contact info appear at the top?
- Are bullet points used in experience sections?
- Is the resume an appropriate length (1-2 pages typically)?

STEP 5 — ACHIEVEMENT QUALITY AUDIT
For each experience bullet in the resume, check:
- Does it start with a strong action verb?
- Does it quantify impact? (numbers, percentages, scale)
- Does it follow CAR format (Context/Action/Result)?
- Score the experience section based on achievement quality, not just keyword presence.

STEP 6 — HONEST SCORING
Score each dimension based on the evidence found in Steps 2-5.
Do NOT round up. A resume missing 5+ required keywords must score below 65 on keywordCoverage.

STEP 7 — REWRITE EXAMPLES
Pick the 2 weakest bullet points from the experience section.
For each: provide the original text, then rewrite it with:
- A strong action verb
- A quantified result (if original had any hint of a number or scale, use it)
- The most important missing keyword from the job ad naturally embedded
- CAR structure

STEP 8 — PRIORITY RECOMMENDATIONS
Generate 4-6 recommendations ordered by impact:
1. Missing critical keywords (HIGH — add these first, they affect ATS immediately)
2. Formatting issues (HIGH if ATS-breaking, MEDIUM otherwise)
3. Achievement rewrites (MEDIUM — improves recruiter impression)
4. Positioning improvements (MEDIUM — reorder or reframe existing content)
5. Nice-to-haves (LOW — polish items)
Each recommendation must be specific ("Add 'Kubernetes' to your Skills section" not "Add more keywords").

STEP 9 — RECRUITER PERSPECTIVE
Write 2-3 sentences as a recruiter's honest internal reaction to this resume for this specific role.
Be direct. Would you shortlist? Why or why not? What's your biggest hesitation?

STEP 10 — FINAL VERDICT
One specific, honest sentence summarizing the overall fit and the single most impactful thing to improve.

${toneNote}
${langNote}

Return EXACTLY this JSON structure:
${SCHEMA}

Return ONLY the JSON. Nothing else.`;
}

// ─── Output validator & repairer ─────────────────────────────────────────────

function extractJSON(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.startsWith('{')) return trimmed;
  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (fenced?.[1]) return fenced[1].trim();
  const start = trimmed.indexOf('{');
  const end = trimmed.lastIndexOf('}');
  if (start !== -1 && end > start) return trimmed.slice(start, end + 1);
  return null;
}

function clamp(v: unknown, min = 0, max = 100): number {
  const n = typeof v === 'number' ? v : Number(v);
  if (isNaN(n)) return 50;
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
      score: clamp(s.score),
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

  return {
    detectedLanguages: {
      resume: ensureString(langs.resume, 'unknown'),
      jobAd: ensureString(langs.jobAd, 'unknown'),
      mismatch: langs.mismatch === true || langs.resume !== langs.jobAd,
    },
    scores: {
      ats: clamp(scores.ats),
      jobMatch: clamp(scores.jobMatch),
      languageAlignment: clamp(scores.languageAlignment),
      readability: clamp(scores.readability),
      keywordCoverage: clamp(scores.keywordCoverage),
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
