/** The resume-analysis user prompt (brief / task / full), with locale conventions. */

import { estimateTokens, getModelTier, truncateResume } from '../context-manager/index.js';
import { resumeConventions } from '../locale/index.js';
import { type PromptTarget, resolveProfile } from '../provider/index.js';
import { SCHEMA, SCHEMA_COMPACT } from './schema.js';

export interface PromptMeta {
  resumeLanguage?: string;
  jobAdLanguage?: string;
  targetLocale?: string;
  outputTone?: string;
  modelName?: string; // For model-aware context management
}

export function buildAnalysisPrompt(
  resume: string,
  jobAd: string,
  meta: PromptMeta = {},
  target?: PromptTarget
): string {
  const resolved = resolveProfile(
    target ?? (meta.modelName ? getModelTier(meta.modelName) : 'large')
  );
  const { depth, schema, truncation, jobAdChars } = resolved;
  const schemaStr = schema === 'compact' ? SCHEMA_COMPACT : SCHEMA;

  const toneNote = meta.outputTone ? `Write all feedback text in a ${meta.outputTone} tone.` : '';
  const langNote =
    meta.targetLocale && meta.targetLocale !== 'en'
      ? `Write all text fields (feedback, recommendations, rewrites, perspectives) in ${meta.targetLocale}.`
      : '';

  // Pre-detected languages (skip LLM detection)
  const langDetectionNote =
    meta.resumeLanguage && meta.jobAdLanguage
      ? `
### PRE-DETECTED LANGUAGES (DO NOT RE-DETECT)
- Resume language: ${meta.resumeLanguage}
- Job ad language: ${meta.jobAdLanguage}
- Language mismatch: ${meta.resumeLanguage !== meta.jobAdLanguage}

Use these pre-detected languages for your analysis. DO NOT perform your own language detection.`
      : '';

  // Section headers + date conventions follow the JOB-AD locale, not a fixed market.
  const conv = resumeConventions(meta.jobAdLanguage ?? meta.targetLocale);
  const marketNote = `
### MARKET CONVENTIONS (job-ad locale: ${meta.jobAdLanguage ?? meta.targetLocale ?? 'unknown'})
Standard section headers for this market: ${conv.headers.summary} / ${conv.headers.experience} / ${conv.headers.education} / ${conv.headers.skills}. Treat these and their direct English equivalents as ATS-standard — do not penalize a resume for using them. Judge date formatting against this market's convention (e.g. ${conv.dateExample}), not a fixed US or German style.`;

  // Truncate the resume per the resolved strategy; cap the job ad by chars.
  const resumeTokens = estimateTokens(resume, meta.jobAdLanguage);
  const r = resumeTokens > truncation.maxTokens ? truncateResume(resume, truncation) : resume;
  const j = jobAd.slice(0, jobAdChars);

  // Compact prompt for small / unknown-local models — fewer steps.
  if (depth === 'brief') {
    return `<candidate_resume>
${r}
</candidate_resume>

<job_ad>
${j}
</job_ad>
${langDetectionNote}
${marketNote}

${toneNote}
${langNote}

Analyze how well the resume matches the job ad. Think step by step before writing the JSON.

Steps:
1. Extract the top 10 required skills/technologies from the job ad
2. Check each one against the resume — matched or missing?
3. Check ATS format: standard section headers, single column, no tables
4. Check achievement quality: action verbs, quantified results
5. Score each dimension 0–100 using the rules in your instructions
6. Write 3 specific recommendations ordered by impact
7. Write an honest recruiter reaction in 2 sentences

Return EXACTLY this JSON and nothing else:
${schemaStr}

Return ONLY the JSON.`;
  }

  // CLI agent — frame as a self-verifying task brief, not a single-shot prompt.
  if (depth === 'task') {
    return `<candidate_resume>
${r}
</candidate_resume>

<job_ad>
${j}
</job_ad>
${langDetectionNote}
${marketNote}

${toneNote}
${langNote}

TASK: analyze the resume against the job ad through your three lenses (ATS, recruiter, strategist). Extract the job requirements, map each to the resume (semantic match), audit ATS formatting and achievement quality, then score every dimension honestly per your acceptance checks. Draft the JSON, validate it against the schema, and revise until every acceptance check passes.

Return a JSON object matching this schema:
${schemaStr}

Return only the JSON object.`;
  }

  return `### RESUME TEXT ###
${r}

### JOB ADVERTISEMENT ###
${j}
${langDetectionNote}
${marketNote}

### ANALYSIS STEPS ###

STEP 1 — LANGUAGE DETECTION
${meta.resumeLanguage && meta.jobAdLanguage ? 'Use the pre-detected languages provided above. DO NOT re-detect.' : 'Detect the language of the resume and the job ad separately.'}

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

STEP 6 — WEIGHTED SCORING (CRITICAL - BE BRUTALLY HONEST)

Calculate each score using the weighted formula:

**ATS Score Calculation:**
1. Count exact keyword matches from job ad (max 35 points)
2. Check section completeness: all 5 standard sections present? (max 25 points)
3. Format compliance audit: single column, no tables, consistent dates (max 20 points)
4. Parsing safety check: no ATS-killer issues (max 20 points)
Total = sum of above (0-100)

**Keyword Coverage Calculation:**
1. Extract ALL required skills/technologies from job ad (make a list)
2. Count exact matches in resume (50 points max)
3. Count semantic matches (30 points max)
4. Count related terms (20 points max)
Total = (matches / total_required) * 100
RULE: Missing 5+ critical keywords = automatic score <65

**Job Match Calculation:**
1. Experience relevance: do past roles match requirements? (40 points max)
2. Seniority alignment: right level for this role? (25 points max)
3. Domain match: same industry/sector? (20 points max)
4. Requirement coverage: % of job requirements met (15 points max)
Total = sum of above (0-100)

**Readability Calculation:**
1. Scannability: bullet format, white space (30 points max)
2. Achievement quality: quantified results, CAR format (30 points max)
3. Action verbs: strong verbs, active voice (20 points max)
4. Length: appropriate for experience level (20 points max)
Total = sum of above (0-100)

**Language Alignment:**
Use the scale defined in system prompt (0-100)

Do NOT round up. Be strict. A score of 64.5 is 64, not 65.

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
${schemaStr}

Return ONLY the JSON. Nothing else.`;
}
