/**
 * Production prompt engineering for AI Generate.
 *
 * Layered architecture:
 * 1. Metadata extraction (JSON) — detect name, role, company, languages, top keywords
 * 2. Resume generation — ATS-optimized, keyword-emphasized, mode-aware
 * 3. Cover letter generation — personalized, tone-aware, keyword-emphasized
 *
 * Keyword emphasis: the AI outputs **keyword** markdown for important terms.
 * The renderer converts these to real bold in DOCX and PDF — never rendered
 * as literal asterisks.
 */

export type GenerationMode =
  | 'ats' // Conservative ATS Optimization
  | 'recruiter' // Recruiter-Friendly Rewrite
  | 'technical' // Technical Role Optimization
  | 'executive' // Executive / Senior Rewrite
  | 'startup' // Startup Tone
  | 'corporate' // Corporate / Enterprise
  | 'localize'; // International Localization

export interface GenerationMeta {
  resumeLanguage: string;
  jobAdLanguage: string;
  mismatch: boolean;
  candidateName: string;
  jobTitle: string;
  companyName: string;
  targetLanguage: string;
  /** Top keywords/technologies extracted from the job ad. Used for bold emphasis. */
  topRequirements: string[];
}

// ─── Mode descriptors ────────────────────────────────────────────────────────

export const MODES: Record<
  GenerationMode,
  { label: string; description: string; toneInstruction: string }
> = {
  ats: {
    label: 'ATS Optimized',
    description: 'Maximize keyword coverage for applicant tracking systems',
    toneInstruction:
      'Optimize for ATS parsing above all else. Use exact keyword phrases from the job ad verbatim in context. Ensure standard section headers: Professional Summary, Work Experience, Education, Skills. Consistent date format throughout. Start every bullet with a strong action verb. Quantify every achievement that can be quantified.',
  },
  recruiter: {
    label: 'Recruiter-Friendly',
    description: 'Optimized for human recruiter 7-second screening',
    toneInstruction:
      'Optimize for the 7-second recruiter scan. Lead with most relevant experience. Every bullet starts with a strong action verb and ends with a measurable result. No walls of text. Professional Summary: 2-3 sentences stating seniority, domain, and value for THIS role.',
  },
  technical: {
    label: 'Technical Role',
    description: 'Highlights technical depth and engineering specifics',
    toneInstruction:
      'Lead with technical depth. Every bullet names specific technologies, architecture decisions, and scale metrics. Show system design thinking. Quantify performance improvements (latency, throughput, uptime, scale). Use precise technical vocabulary from the job ad.',
  },
  executive: {
    label: 'Executive / Senior',
    description: 'Leadership-focused, strategic and high-level',
    toneInstruction:
      'Lead with organizational impact. Every bullet answers: what changed, and what was the business outcome? Emphasize team size, budgets, revenue/cost impact, strategic initiatives. Remove tactical details. Use executive vocabulary: drove, built, transformed, scaled, led.',
  },
  startup: {
    label: 'Startup Tone',
    description: 'Modern, dynamic, growth-oriented language',
    toneInstruction:
      'Write for a startup reader who values velocity, ownership, and raw impact. Modern active language. Highlight things built from scratch, delivery speed, cross-functional ownership, growth metrics. Avoid corporate language.',
  },
  corporate: {
    label: 'Corporate / Enterprise',
    description: 'Formal, structured, compliance-ready',
    toneInstruction:
      'Formal enterprise tone. Precise and structured. Emphasize process adherence, stakeholder management, cross-functional collaboration, risk management, governance. Remove casual phrasing.',
  },
  localize: {
    label: 'Localized Output',
    description: 'Culturally adapted for the target market',
    toneInstruction:
      'Write natively in the target language. Do NOT translate literally — fully adapt for the target market. Use local resume conventions and market-expected terminology.',
  },
};

// ─── Keyword emphasis helpers ─────────────────────────────────────────────────

/**
 * Build the bold emphasis instruction block for prompts.
 * The AI uses **keyword** notation; the renderer converts to real bold.
 */
function buildEmphasisBlock(keywords: string[]): string {
  if (!keywords.length) return '';
  const list = keywords
    .slice(0, 12)
    .map((k) => `**${k}**`)
    .join(', ');
  return `
KEYWORD EMPHASIS — CRITICAL:
Wrap the following job-ad keywords in **double asterisks** when they appear naturally in your output:
${list}

Emphasis rules:
- Bold ONLY when the keyword appears in a genuinely relevant technical or skill context
- Bold the FIRST occurrence per section — not every instance
- Maximum 2–3 bolded terms per bullet point
- NEVER bold: company names, dates, pronouns, generic verbs, or section headers
- Bolding should feel strategic and natural — not keyword-stuffed
- The **asterisks** will be converted to real bold typography in the exported document

Example:
  WEAK:  Built frontend applications with React and TypeScript
  GOOD:  Built scalable **React** and **TypeScript** frontend applications integrated with **REST APIs**`;
}

// ─── Metadata extraction prompt ──────────────────────────────────────────────

export function buildMetadataPrompt(
  resume: string,
  jobAd: string
): { system: string; user: string } {
  return {
    system: `You are a document parser. Extract structured data from resumes and job ads. Return ONLY valid JSON. No prose. No markdown.`,
    user: `Extract from the resume and job ad below.

### RESUME ###
${resume.slice(0, 3000)}

### JOB AD ###
${jobAd.slice(0, 2000)}

Return this exact JSON (no other text):
{
  "candidateName": "full name from resume or empty string",
  "jobTitle": "exact job title from job ad",
  "companyName": "company name from job ad or empty string",
  "resumeLanguage": "ISO 639-1 code e.g. en, de, fr",
  "jobAdLanguage": "ISO 639-1 code e.g. en, de, fr",
  "topRequirements": ["up to 12 exact technology names and skills from the job ad that should be bolded — prefer specific names like React, TypeScript, AWS, Kubernetes over generic terms like communication or teamwork"],
  "candidateSeniority": "junior|mid|senior|lead|executive"
}

Return ONLY the JSON object.`,
  };
}

// ─── Resume system prompt ─────────────────────────────────────────────────────

export function buildResumeSystemPrompt(mode: GenerationMode): string {
  const modeInstr = MODES[mode].toneInstruction;

  return `You are an expert Resume Writer with deep knowledge of ATS systems, recruiter behavior, and modern hiring practices.

CORE RULES — NEVER BREAK:
1. NEVER invent skills, technologies, employers, dates, or achievements not in the original resume
2. You MAY improve wording, reorder content, and reframe existing facts for maximum impact
3. ONLY add keywords from the job ad when they can be embedded naturally into EXISTING true statements
4. Every bullet point must refer to work the candidate actually did

FORMATTING RULES — both ATS and recruiters require these:
- Standard section headers ONLY: "Professional Summary", "Work Experience", "Education", "Skills", "Certifications", "Projects"
- Date format MUST be consistent throughout: "Month YYYY – Month YYYY" or "YYYY – YYYY"
- Every experience bullet starts with a strong past-tense action verb
- Each bullet: max 2 lines. No paragraph-style bullets.
- Quantify EVERY achievement that can be quantified
- Skills: comma-separated, grouped by category (Languages, Frameworks, Tools, Platforms)
- Single column ONLY — no tables, no columns, no headers/footers (ATS requirement)

BULLET QUALITY — CAR FORMAT mandatory:
  WEAK:  "Responsible for backend API development"
  STRONG: "Architected **REST API** serving 200k daily requests, reducing response time 45% via **Redis** caching"
Action verb + what + with which technology (bolded) + measurable result.

KEYWORD NATURALIZATION:
Instead of: "Built frontend applications"
Write:       "Built scalable **React** and **TypeScript** frontend applications integrated with **REST APIs**"

The keywords must be woven into the natural sentence — not tacked on.

MODE: ${MODES[mode].label}
${modeInstr}

OUTPUT:
Plain text with **double asterisks** for keyword emphasis (renderer converts to real bold).
Standard section headers, "•" for bullets.
No markdown other than **bold**. No explanations. Output ONLY the resume.`;
}

// ─── Resume user prompt ───────────────────────────────────────────────────────

export function buildResumePrompt(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  _mode: GenerationMode
): string {
  const langNote = meta.mismatch
    ? `IMPORTANT: The resume is in ${meta.resumeLanguage} but the job ad is in ${meta.jobAdLanguage}. Rewrite entirely in ${meta.targetLanguage} using job market terminology native to that market.`
    : `Write in ${meta.targetLanguage}.`;

  const emphasisBlock = buildEmphasisBlock(meta.topRequirements ?? []);

  return `### ORIGINAL RESUME ###
${resume.slice(0, 5000)}

### TARGET JOB ADVERTISEMENT ###
${jobAd.slice(0, 2500)}

### CONTEXT ###
Candidate: ${meta.candidateName || 'Unknown'}
Target Role: ${meta.jobTitle || 'Unknown'}
Company: ${meta.companyName || 'Unknown'}
${langNote}
${emphasisBlock}

### REWRITING PROCESS ###

PHASE 1 — INTERNAL ANALYSIS (do not output):
1. Extract the 8–10 most important requirements from the job ad
2. Map each requirement to the candidate's existing experience
3. Identify the 2–3 experience items most relevant to this role
4. Note which bullets lack quantification or strong action verbs
5. List experience to minimize (irrelevant to this role)

PHASE 2 — REWRITE:

Professional Summary (3 sentences max):
- Sentence 1: Seniority + domain + years of experience + specific value for THIS role
- Sentence 2: Top 1-2 relevant technical strengths (use bolded keywords)
- Sentence 3: A specific career achievement or differentiator
- Include the job title from the ad naturally

Work Experience (most recent first):
- Reorder bullets: most relevant to this job first
- Rewrite weak bullets to CAR format: Action Verb + What + Technology (bolded) + Result
- Embed bolded keywords naturally into EXISTING true statements
- Compress or remove bullets irrelevant to this role
- Each role: 3–5 strong bullets max

Skills Section:
- Order by relevance to this job ad (most relevant first)
- Group: Languages | Frameworks | Tools | Platforms | Methodologies
- Bold the skills that also appear in topRequirements

PHASE 3 — ATS VERIFICATION:
Before outputting, verify:
✓ All section headers are standard
✓ Dates are consistent throughout
✓ Every bullet starts with action verb
✓ Key job-ad technologies appear bolded and naturally integrated
✓ No tables, columns, or special chars that break ATS parsers

CRITICAL: Only use facts from the original resume.
Output the complete rewritten resume now:`;
}

// ─── Cover letter system prompt ───────────────────────────────────────────────

export function buildCoverLetterSystemPrompt(mode: GenerationMode): string {
  const modeInstr = MODES[mode].toneInstruction;

  return `You are a cover letter specialist who writes letters that get read — not filtered out.

WHAT KILLS COVER LETTERS (never do these):
- Opening with "I am excited to apply for..." or "I am writing to express my interest..."
- Using: passionate, hard-working, team player, go-getter, synergy, leverage
- Repeating the resume in paragraph form
- Generic paragraphs applicable to any company
- Ending with "I hope to hear from you soon" or "Thank you for your consideration"

WHAT MAKES COVER LETTERS WORK:
- First sentence: immediate, specific value for THIS role — not a generic opener
- References something specific from the job ad (shows genuine reading)
- Connects 1–2 specific past achievements to 1–2 specific job requirements
- Shows genuine understanding of the company/role context
- Ends with confidence — not desperation
- Keywords from the job ad bolded with **asterisks** where they appear naturally

COMPLETE STRUCTURE:
[Candidate Name]
[Email] | [Phone if in resume] | [City if in resume]
[Date]

[Company Name]
[Hiring Team / Manager name if in job ad]

Dear [Hiring Team / specific name],

[HOOK: One sentence. State the specific value you bring to this role. Start with your value, not "I". Reference the job title.]

[EVIDENCE: 2–3 sentences. Pick 1–2 achievements from the resume that directly prove you can do the top requirements. Include specific bolded technology/skill names and measurable results.]

[FIT: 2 sentences. Show you understand what this company/team is actually trying to accomplish. Reference something concrete from the job ad. Connect your professional goals to their needs.]

[CLOSE: 2 sentences. Confident invitation to discuss. No desperation. No begging.]

[Kind regards / appropriate closing in target language]
[Candidate Name]

RULES:
1. Never invent experience, metrics, or skills not in the resume
2. Use the actual company name and job title
3. Total body: 200–300 words
4. Include candidate contact info in header if in resume
5. Bold keywords with **asterisks** (max 4–6 per letter)

MODE: ${MODES[mode].label}
${modeInstr}

OUTPUT: Complete cover letter with header, date, addressee, salutation, 4 paragraphs, sign-off.
Use **double asterisks** for keyword emphasis. Plain text otherwise. Output the letter only.`;
}

// ─── Cover letter user prompt ─────────────────────────────────────────────────

export function buildCoverLetterPrompt(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  _mode: GenerationMode
): string {
  const today = new Date().toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });

  const langNote = meta.mismatch
    ? `Write entirely in ${meta.targetLanguage}. Use native phrasing and professional conventions for that market. Do NOT translate literally.`
    : `Write in ${meta.targetLanguage}.`;

  const emphasisBlock = buildEmphasisBlock(meta.topRequirements ?? []);

  return `### CANDIDATE RESUME ###
${resume.slice(0, 4000)}

### JOB ADVERTISEMENT ###
${jobAd.slice(0, 2500)}

### CONTEXT ###
Candidate: ${meta.candidateName || 'Unknown'}
Role: ${meta.jobTitle || 'this role'} at ${meta.companyName || 'this company'}
Today: ${today}
${langNote}
${emphasisBlock}

### WRITING PROCESS ###

STEP 1 — ANALYZE:
- What are the top 3 requirements in the job ad?
- What does this company/team actually need to accomplish?
- Which 1–2 resume achievements best prove the candidate can deliver?
- What specific technologies/tools are emphasized in the job ad?

STEP 2 — WRITE the complete cover letter following your system instructions:
- Header (name, contact, date)
- Addressee block (company, hiring team)
- Salutation
- Hook: value-first opener, NOT "I am applying"
- Evidence: specific achievement + bolded technology + result
- Fit: show you understand the company's actual goals
- Confident close
- Sign-off appropriate to language/market

STEP 3 — VERIFY before outputting:
✓ First sentence does NOT start with "I am excited/applying/writing"
✓ Company name appears in the body
✓ At least one specific metric or achievement from the resume is referenced
✓ At least one job-ad requirement is directly addressed with a bolded keyword
✓ 200–300 words in the body

CRITICAL: Only use facts from the resume.
Write the complete cover letter now:`;
}

// ─── Utilities ────────────────────────────────────────────────────────────────

export function extractPlainText(raw: string): string {
  return raw
    .replace(/^#{1,6}\s/gm, '')
    .replace(/\*\*\*(.+?)\*\*\*/g, '**$1**') // triple → double (preserve bold)
    .replace(/\*([^*]+)\*/g, '$1') // single italic → plain
    .replace(/`(.+?)`/g, '$1')
    .replace(/```[\s\S]*?```/g, '')
    .trim();
}

export function validateMetadata(raw: string): GenerationMeta | null {
  try {
    const jsonStr = raw.slice(raw.indexOf('{'), raw.lastIndexOf('}') + 1);
    const parsed = JSON.parse(jsonStr);
    return {
      candidateName: parsed.candidateName ?? '',
      jobTitle: parsed.jobTitle ?? '',
      companyName: parsed.companyName ?? '',
      resumeLanguage: parsed.resumeLanguage ?? 'en',
      jobAdLanguage: parsed.jobAdLanguage ?? 'en',
      mismatch: (parsed.resumeLanguage ?? 'en') !== (parsed.jobAdLanguage ?? 'en'),
      targetLanguage: parsed.jobAdLanguage ?? parsed.resumeLanguage ?? 'en',
      topRequirements: Array.isArray(parsed.topRequirements) ? parsed.topRequirements : [],
    };
  } catch {
    return null;
  }
}
