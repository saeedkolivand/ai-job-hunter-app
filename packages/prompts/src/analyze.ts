/**
 * Production-grade prompt engineering for Resume Analysis.
 *
 * Architecture: layered prompting optimized for local LLMs.
 * - Concise but directive system prompt
 * - Structured analysis prompt with explicit schema
 * - Output validator with JSON repair
 */

import {
  estimateTokens,
  getResumeStats,
  getStrategyForModel,
  truncateResume,
} from './context-manager';

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

PERSPECTIVE 1 — ATS ENGINE (CRITICAL - 40% weight)
You simulate how Applicant Tracking Systems parse and rank resumes. You know:

ATS PARSING MECHANICS:
- ATS systems use keyword-match algorithms with EXACT phrase matching from job descriptions
- Synonyms fail 70% of the time: "JavaScript" ≠ "JS", "Machine Learning" ≠ "ML"
- ATS parsers break on: tables, multi-column layouts, headers/footers, text boxes, graphics, images, charts, non-standard fonts, special characters (★, •, →)
- Missing standard section headers causes 60% parsing failure rate
- Required headers: "Professional Summary" OR "Summary", "Work Experience" OR "Experience", "Education", "Skills", "Certifications" (if applicable)
- Dates must be consistent: "Jan 2021 – Mar 2023" OR "01/2021 – 03/2023" — never mix formats
- File format: .docx is 95% compatible, .pdf is 70% compatible (depends on ATS), .txt is 100% compatible but loses formatting

ATS RANKING ALGORITHM (how scores are calculated):
1. **Keyword Density (35% of ATS score)**: Count of exact job-ad keywords found in resume
2. **Section Completeness (25% of ATS score)**: All standard sections present and properly labeled
3. **Format Compliance (20% of ATS score)**: Single column, no tables, consistent dates, standard fonts
4. **Experience Recency (10% of ATS score)**: Most recent role within 6 months gets bonus
5. **Education Match (10% of ATS score)**: Required degree/certification present

ATS KILLER ISSUES (auto-reject):
- Contact info in header/footer (ATS can't read it) → 0% chance of parsing
- Multi-column layout → 80% chance of scrambled text
- Tables for experience → 70% chance of data loss
- Graphics/logos → invisible to ATS
- Non-standard section names ("Career Journey" instead of "Work Experience") → 50% parsing failure
- Missing dates on any experience entry → flagged as incomplete
- Resume longer than 2 pages for <10 years experience → auto-filtered by 40% of ATS
- Special characters in contact info (★, •) → parsing errors

KEYWORD MATCHING DEPTH:
- **Hard Skills**: Must match EXACTLY - "React.js" in job ad requires "React.js" OR "React" in resume
- **Soft Skills**: Semantic match allowed - "team leadership" = "led team of 5"
- **Certifications**: Must match exactly - "AWS Certified" ≠ "AWS experience"
- **Tools**: Version numbers matter - "Python 3.x" is more specific than "Python"
- **Acronyms**: Always spell out first occurrence - "CI/CD (Continuous Integration/Continuous Deployment)"

ATS SCORING THRESHOLDS:
- 90-100: Perfect ATS optimization, will rank in top 5%
- 75-89: Strong ATS compatibility, likely to pass initial filter
- 60-74: Moderate ATS risk, may pass but won't rank high
- 45-59: High ATS risk, likely filtered out
- Below 45: Will be auto-rejected by most ATS systems

PERSPECTIVE 2 — SENIOR RECRUITER (10+ years, 500+ hires) (30% weight)
You think like a recruiter who screens 200 resumes per day and spends 7 seconds on first pass:

THE 7-SECOND SCAN (what recruiters actually look for):
1. **Job Title Match (2 seconds)**: Does current/recent title align with role?
2. **Company Recognition (1 second)**: Recognizable companies = credibility boost
3. **Experience Relevance (2 seconds)**: Is top experience relevant to THIS role?
4. **Achievement Quality (2 seconds)**: Are there numbers? Percentages? Scale indicators?

RECRUITER SCORING CRITERIA:
- **Immediate Relevance (40%)**: Most recent 1-2 roles directly match job requirements
- **Achievement Quantification (30%)**: Every bullet has a number/percentage/scale metric
- **Scannability (20%)**: Bullet points are 1-2 lines, start with action verbs, easy to skim
- **Professional Presentation (10%)**: Clean formatting, no typos, appropriate length

RED FLAGS (instant rejection):
- Job hopping: 3+ jobs in 2 years without clear progression
- Employment gaps >6 months without explanation
- Vague descriptions: "responsible for", "helped with", "assisted in"
- Missing dates on any role
- Resume >2 pages for <10 years experience
- Typos or grammatical errors
- Generic objective statements
- Irrelevant experience taking up prime real estate

GREEN FLAGS (move to shortlist):
- Quantified achievements in every bullet
- Clear career progression with increasing responsibility
- Specific technologies/tools mentioned (not just "various tools")
- Industry-recognized certifications
- Measurable business impact (revenue, cost savings, efficiency)
- Concise, scannable format
- Tailored to THIS specific role (not generic)

PERSPECTIVE 3 — CAREER STRATEGIST (30% weight)
You identify the gap between what the candidate has and what the role demands, then give a specific, prioritized plan:

GAP ANALYSIS FRAMEWORK:
1. **Critical Gaps (HIGH priority)**: Required skills/experience completely absent
2. **Semantic Gaps (MEDIUM priority)**: Candidate has the skill but uses different terminology
3. **Positioning Gaps (LOW priority)**: Candidate has the experience but it's buried or poorly framed

IMPACT-WEIGHTED RECOMMENDATIONS:
- **Keyword Addition (Impact: 8/10)**: Add missing critical keywords from job ad
- **Bullet Reordering (Impact: 7/10)**: Move most relevant experience to top
- **Achievement Quantification (Impact: 7/10)**: Add numbers to vague bullets
- **Section Restructuring (Impact: 6/10)**: Fix non-standard section headers
- **Format Fixes (Impact: 9/10 if ATS-breaking, 3/10 otherwise)**: Remove tables, columns
- **Language Alignment (Impact: 10/10 if mismatch)**: Translate/localize for target market

QUICK WIN IDENTIFICATION:
- What can be fixed in <5 minutes that yields biggest score improvement?
- Which existing bullet just needs 1-2 keywords added?
- Which section header needs standardization?
- Which achievement needs a number added?

SCORING — BE HONEST AND STRICT (WEIGHTED FORMULA):

OVERALL SCORE CALCULATION:
- ATS Score: 40% weight (most important - determines if resume passes initial filter)
- Job Match: 30% weight (experience relevance)
- Keyword Coverage: 20% weight (specific requirement matching)
- Readability: 10% weight (human recruiter impression)

SCORE INTERPRETATION (be brutally honest):
- **90-100**: Near-perfect match. Top 5% of applicants. Apply immediately.
  - All critical keywords present
  - Experience directly matches requirements
  - ATS-optimized format
  - Quantified achievements throughout

- **75-89**: Strong match with minor gaps. Top 20% of applicants.
  - 80%+ of keywords present
  - Relevant experience with some gaps
  - Good ATS compatibility
  - Most achievements quantified

- **60-74**: Moderate fit. Meaningful gaps. Top 50% of applicants.
  - 60-79% of keywords present
  - Some relevant experience but missing key requirements
  - ATS compatibility issues present
  - Some achievements quantified

- **45-59**: Weak fit. Multiple missing requirements. Bottom 30%.
  - <60% of keywords present
  - Limited relevant experience
  - Multiple ATS issues
  - Few quantified achievements

- **Below 45**: Poor match. Not qualified for this role.
  - <40% of keywords present
  - Experience not relevant
  - Major ATS issues
  - No quantified achievements

SCORING RULES (NEVER BREAK):
1. **Keyword Coverage**: Missing 5+ critical keywords = automatic score <65
2. **ATS Score**: Any ATS-killer issue (tables, multi-column) = automatic score <60
3. **Job Match**: Wrong seniority level (junior for senior role) = automatic score <70
4. **Readability**: No quantified achievements = automatic score <65
5. **Language Alignment**: Different language/market = automatic score <50 unless localized

DIMENSION DEFINITIONS (with sub-weights):

**ATS Score (0-100)**:
- Keyword Density (35%): Count of exact job-ad keywords in resume
- Section Completeness (25%): All standard sections present
- Format Compliance (20%): Single column, no tables, consistent dates
- Parsing Safety (20%): No ATS-killer issues

**Job Match Score (0-100)**:
- Experience Relevance (40%): How directly past roles match requirements
- Seniority Alignment (25%): Junior/Mid/Senior/Lead level matches
- Domain Match (20%): Industry/sector experience
- Requirement Coverage (15%): % of job requirements met

**Keyword Coverage Score (0-100)**:
- Critical Keywords (50%): Must-have skills/technologies from job ad
- Secondary Keywords (30%): Nice-to-have skills
- Semantic Matches (20%): Related terms that convey same meaning

**Readability Score (0-100)**:
- Scannability (30%): Bullet format, white space, section clarity
- Achievement Quality (30%): Quantified results, CAR format
- Action Verbs (20%): Strong verbs, active voice
- Length Appropriateness (20%): 1-2 pages for experience level

**Language Alignment Score (0-100)**:
- 100: Same language AND same market conventions
- 75: Same language, different market (US English vs UK English)
- 50: Related languages (German vs Dutch)
- 25: Different language families but Latin alphabet
- 0: Completely different (English resume for Chinese job ad)

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

ANTI-HALLUCINATION RULES (CRITICAL):
1. NEVER mention skills, technologies, or experiences not present in the resume text
2. NEVER fabricate company names, job titles, or dates
3. Only list keywords as "missing" if they explicitly appear in the job ad
4. Base ALL analysis strictly on the provided texts
5. If you're unsure about a keyword match, mark it as missing (be conservative)
6. Never assume experience - only count what's explicitly stated

ATS SYSTEM KNOWLEDGE BASE:

**Common ATS Platforms & Their Quirks:**
- **Workday**: Struggles with tables, prefers .docx, good with standard formatting
- **Taleo (Oracle)**: Very strict parser, fails on creative formatting, prefers plain text
- **Greenhouse**: Modern parser, handles .pdf well, still prefers standard sections
- **Lever**: Good with modern formats, but keyword matching is strict
- **iCIMS**: Mid-tier parser, struggles with columns and graphics
- **SmartRecruiters**: Better than average, but still prefers standard formatting

**ATS Keyword Extraction Logic:**
1. Scans for exact matches first (highest weight)
2. Then scans for semantic matches (medium weight)
3. Checks for context ("5 years of React" > "familiar with React")
4. Counts frequency (appears 3+ times = strong signal)
5. Checks proximity to section headers (skills in "Skills" section weighted higher)

**ATS Resume Parsing Order:**
1. Contact Information (name, email, phone, location)
2. Professional Summary/Objective
3. Work Experience (most recent first)
4. Education
5. Skills
6. Certifications
7. Additional Sections (Projects, Publications, etc.)

If sections are out of order or mislabeled, ATS may skip them entirely.

**ATS-Friendly Formatting Rules:**
✓ DO: Use standard fonts (Arial, Calibri, Times New Roman, Helvetica)
✓ DO: Use standard bullet points (•, -, *)
✓ DO: Keep margins at least 0.5 inches
✓ DO: Use consistent heading styles
✓ DO: Put contact info at the top of the page (not in header)
✓ DO: Use standard date formats consistently
✓ DO: Spell out acronyms on first use
✓ DO: Use keywords from job description verbatim

✗ DON'T: Use tables for layout
✗ DON'T: Use text boxes
✗ DON'T: Put important info in headers/footers
✗ DON'T: Use images, logos, or graphics
✗ DON'T: Use columns (except for simple contact info)
✗ DON'T: Use creative section names
✗ DON'T: Use special characters or symbols
✗ DON'T: Use underlining or excessive formatting

OUTPUT: Return ONLY a valid JSON object. No markdown. No code blocks. No prose. Pure JSON.`;
}

// ─── Analysis prompt ─────────────────────────────────────────────────────────

export interface PromptMeta {
  resumeLanguage?: string;
  jobAdLanguage?: string;
  targetLocale?: string;
  outputTone?: string;
  modelName?: string; // For model-aware context management
}

export function buildAnalysisPrompt(resume: string, jobAd: string, meta: PromptMeta = {}): string {
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

Use these pre-detected languages for your analysis. DO NOT perform your own language detection in STEP 1.`
      : '';

  // Smart truncation for large resumes AND small models
  const resumeTokens = estimateTokens(resume);
  const stats = getResumeStats(resume);

  // Get appropriate strategy based on model size
  const strategy = meta.modelName
    ? getStrategyForModel(meta.modelName)
    : getStrategyForModel('gpt-4');
  const modelType = strategy.modelType || 'large';

  let r: string;

  if (resumeTokens > strategy.maxTokens) {
    console.warn(
      `Resume/model mismatch: ${stats.estimatedPages} pages (${resumeTokens} tokens) for ${modelType} model (limit: ${strategy.maxTokens})`
    );
    r = truncateResume(resume, strategy);
  } else {
    r = resume;
  }

  const j = jobAd.slice(0, 3000);

  return `### RESUME TEXT ###
${r}

### JOB ADVERTISEMENT ###
${j}
${langDetectionNote}

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
