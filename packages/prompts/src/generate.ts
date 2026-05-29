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

// ─── Link extraction helper ───────────────────────────────────────────────────

// Known social/portfolio domains that belong in a resume contact line.
const PROFILE_DOMAINS = [
  'linkedin.com',
  'github.com',
  'gitlab.com',
  'twitter.com',
  'x.com',
  'behance.net',
  'dribbble.com',
  'medium.com',
  'stackoverflow.com',
  'dev.to',
  'codepen.io',
  'youtube.com',
  'youtu.be',
  'notion.so',
  'figma.com',
  'npmjs.com',
  'crates.io',
  'solo.to',
  'bio.link',
  'linktr.ee',
  'bento.me',
];

function isProfileUrl(url: string): boolean {
  try {
    const host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
    return PROFILE_DOMAINS.some((d) => host === d || host.endsWith(`.${d}`));
  } catch {
    return false;
  }
}

/**
 * Derive a friendly label from a URL — mirrors the Rust url_label() in links.rs.
 * Used when a PDF annotation stores the raw URL as its anchor text instead of a label.
 * Exported for the cross-language parity test against Rust url_label().
 */
export function urlToFriendlyLabel(url: string): string {
  try {
    const host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
    if (host.startsWith('linkedin.com')) return 'LinkedIn';
    if (host.startsWith('github.com')) return 'GitHub';
    if (host.startsWith('gitlab.com')) return 'GitLab';
    if (host.startsWith('twitter.com') || host.startsWith('x.com')) return 'Twitter';
    if (host.startsWith('behance.net')) return 'Behance';
    if (host.startsWith('dribbble.com')) return 'Dribbble';
    if (host.startsWith('medium.com')) return 'Medium';
    if (host.startsWith('stackoverflow.com')) return 'Stack Overflow';
    if (host.startsWith('dev.to')) return 'Dev.to';
    if (host.startsWith('codepen.io')) return 'CodePen';
    if (host.startsWith('youtube.com') || host.startsWith('youtu.be')) return 'YouTube';
    if (host.startsWith('notion.so')) return 'Notion';
    if (host.startsWith('figma.com')) return 'Figma';
    if (host.startsWith('npmjs.com')) return 'npm';
    if (host.startsWith('crates.io')) return 'crates.io';
    // Unknown domain: the bare host (www-stripped, no path). Mirrors the Rust
    // url_label() fallback exactly so the two implementations cannot drift — see
    // the parity test (fixtures/url-labels.json, cargo test export::links).
    return host;
  } catch {
    return url;
  }
}

interface ParsedResumeLinks {
  /** Compact block to inject before <candidate_resume> */
  block: string;
  /** Clean email address extracted from mailto annotation, or empty string */
  cleanEmail: string;
}

/** Generic label for a single non-platform personal site / portfolio URL. */
const WEBSITE_LABEL = 'Website';

interface LinkBlockEntry {
  anchor: string;
  url: string;
}

/**
 * Parse the `\n---\n` markdown reference block (appended by the Rust extractor)
 * into raw `[anchor](url)` entries, in document order. Returns [] when absent.
 */
function parseLinkBlock(resume: string): LinkBlockEntry[] {
  const sep = resume.lastIndexOf('\n---\n');
  if (sep === -1) return [];
  const block = resume.slice(sep + 5);
  const entries: LinkBlockEntry[] = [];
  for (const l of block.split('\n')) {
    if (!l.startsWith('- [')) continue;
    const m = l.match(/^- \[([^\]]+)\]\(([^)]+)\)$/);
    if (!m) continue;
    const anchor = m[1] ?? '';
    const url = m[2] ?? '';
    if (anchor && url) entries.push({ anchor, url });
  }
  return entries;
}

/**
 * Resolve the reference block into ordered contact links for the header line.
 *
 * Every known platform link keeps its brand label. In addition, the FIRST
 * non-platform http(s) URL is admitted ONCE under a generic "Website" label —
 * this is the website/portfolio fix: previously such URLs were dropped wholesale
 * by the PROFILE_DOMAINS allowlist. Subsequent non-platform URLs are still
 * dropped, so a single header-scoped site is surfaced without letting arbitrary
 * inline body URLs leak in. `mailto:` is excluded here (handled separately as the
 * clean email).
 *
 * Both getLinkMap() (post-generation injection) and parseLinksFromResume()
 * (prompt instruction) build on this, so the label the AI is told to write and
 * the label injection later looks for can never drift.
 */
function resolveContactLinks(resume: string): { label: string; url: string }[] {
  const out: { label: string; url: string }[] = [];
  let websiteAdmitted = false;
  for (const { anchor, url } of parseLinkBlock(resume)) {
    if (url.startsWith('mailto:')) continue;
    if (isProfileUrl(url)) {
      // PDFs often store the raw URL as the anchor; derive the friendly label
      // (e.g. "LinkedIn") so injection matches what the AI writes.
      const label = /^https?:\/\//i.test(anchor) ? urlToFriendlyLabel(anchor) : anchor;
      out.push({ label, url });
    } else if (!websiteAdmitted && /^https?:\/\//i.test(url)) {
      out.push({ label: WEBSITE_LABEL, url });
      websiteAdmitted = true;
    }
  }
  return out;
}

/**
 * Build a label→url map for the contact links in the extracted reference block.
 * Used for post-processing: replacing plain labels with [label](url) markdown.
 */
export function getLinkMap(resume: string): Record<string, string> {
  const map: Record<string, string> = {};
  for (const { label, url } of resolveContactLinks(resume)) {
    map[label] = url;
  }
  return map;
}

/**
 * Post-process AI-generated resume/cover-letter text.
 * Scans the first 6 lines for the contact line (contains |) and replaces
 * known profile labels (e.g. "LinkedIn") with [LinkedIn](https://...) so the
 * Rust renderer can attach the hyperlink without displaying the full URL.
 */
export function injectLinksIntoGeneratedText(
  text: string,
  linkMap: Record<string, string>
): string {
  if (!Object.keys(linkMap).length) return text;
  const lines = text.split('\n');
  for (let i = 0; i < Math.min(lines.length, 6); i++) {
    const line = lines[i] ?? '';
    if (!line.includes('|')) continue;
    if (/^(PROFESSIONAL|WORK|EDUCATION|SKILLS|SUMMARY)/i.test(line.trim())) continue;
    let newLine = line;
    for (const [label, url] of Object.entries(linkMap)) {
      // Match the label at word boundaries; skip if already inside a markdown link [...]
      const esc = label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      newLine = newLine.replace(new RegExp(`(?<!\\[)\\b${esc}\\b`, 'gi'), `[${label}](${url})`);
    }
    if (i < lines.length) lines[i] = newLine;
  }
  return lines.join('\n');
}

/**
 * Parse the markdown reference block appended by the Rust PDF/DOCX extractor.
 * Returns a prompt injection block telling the AI to write short labels
 * (LinkedIn, GitHub) — not full URLs. Actual hyperlinks are injected
 * post-generation by injectLinksIntoGeneratedText().
 */
export function parseLinksFromResume(resume: string): ParsedResumeLinks {
  const entries = parseLinkBlock(resume);
  if (!entries.length) return { block: '', cleanEmail: '' };

  const mailto = entries.find((e) => e.url.startsWith('mailto:'));
  const cleanEmail = mailto ? mailto.url.slice('mailto:'.length) : '';

  // Exactly the labels (platform brands + one "Website") getLinkMap() will inject,
  // so the AI is instructed to write the same short labels we later hyperlink.
  const labelEntries = resolveContactLinks(resume).map((e) => e.label);

  if (!labelEntries.length && !cleanEmail) return { block: '', cleanEmail: '' };

  const parts: string[] = [];
  if (cleanEmail) {
    parts.push(`CANDIDATE EMAIL (use this exact address, no spaces): ${cleanEmail}`);
  }
  if (labelEntries.length) {
    parts.push(
      `CANDIDATE PROFILE LINKS — write ONLY these short labels in the contact line (NOT the full URL):\n` +
        labelEntries.join(', ') +
        `\nExample: Haarlem, Netherlands | name@example.com | +31... | LinkedIn | GitHub | Website`
    );
  }

  return { block: parts.join('\n\n'), cleanEmail };
}

/**
 * Strip the link reference block from resume text before sending to the AI
 * so the body text budget is not wasted on the reference list.
 */
function stripLinkBlock(resume: string): string {
  const sep = resume.lastIndexOf('\n---\n');
  return sep === -1 ? resume : resume.slice(0, sep);
}

// ─── Metadata extraction prompt ──────────────────────────────────────────────

export function buildMetadataPrompt(
  resume: string,
  jobAd: string,
  tier: 'large' | 'medium' | 'small' = 'large'
): { system: string; user: string } {
  // One-shot example appended for small models — dramatically improves JSON compliance
  const oneShot =
    tier === 'small'
      ? `\nExample output:\n{"candidateName":"Jane Smith","jobTitle":"Senior Frontend Engineer","companyName":"Acme Corp","resumeLanguage":"en","jobAdLanguage":"en","topRequirements":["React","TypeScript","GraphQL"],"candidateSeniority":"senior"}\n`
      : '';

  const { block: linksBlock } = parseLinksFromResume(resume);
  const resumeBody = stripLinkBlock(resume);

  return {
    system: `You are a document parser. Extract structured data from resumes and job ads. Return ONLY valid JSON. No prose. No markdown.`,
    user: `Extract from the resume and job ad below.
${linksBlock ? `\n${linksBlock}\n` : ''}
<candidate_resume>
${resumeBody.slice(0, 3000)}
</candidate_resume>

<job_ad>
${jobAd.slice(0, 2000)}
</job_ad>

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
${oneShot}
Return ONLY the JSON object.`,
  };
}

// ─── Resume system prompt ─────────────────────────────────────────────────────

export function buildResumeSystemPrompt(
  mode: GenerationMode,
  tier: 'large' | 'medium' | 'small' = 'large'
): string {
  const modeInstr = MODES[mode].toneInstruction;

  if (tier === 'small') {
    const emphasisNote = `Wrap important job-ad keywords in **double asterisks** when they appear naturally (e.g. **React**, **TypeScript**). Max 2–3 bolded terms per bullet.`;
    return `You are an expert resume writer. Rewrite the candidate's resume for the target job.

NEVER BREAK THESE RULES:
1. NEVER invent skills, technologies, employers, dates, or achievements not in the original resume
2. NEVER copy phrases from the job ad as if the candidate wrote them
3. ONLY add keywords from the job ad when they embed naturally into EXISTING true statements
4. Every bullet: Action Verb + What + Technology + Measurable Result (if number exists in original)
5. Every skill, job title, company, date, and achievement MUST come from the original resume

REQUIRED SECTION HEADERS (exact spelling):
Professional Summary · Work Experience · Education · Skills
Optional: Certifications · Projects

DATE FORMAT: "January 2021 – March 2023" or "Jan 2021 – Mar 2023" — consistent throughout.

${emphasisNote}

MODE: ${MODES[mode].label}
${modeInstr}

OUTPUT: Plain text. Standard section headers. Bullets start with •. No markdown except **bold**. Output ONLY the resume.

FINAL CHECK — read your output and confirm:
✓ No skill appears that is not in the original resume
✓ No phrase was copied from the job ad verbatim`;
  }

  return `You are an expert Resume Writer with deep knowledge of ATS systems, recruiter behavior, and modern hiring practices.

Your resume rewrites achieve 90%+ ATS pass rates and 3x higher callback rates.

CORE RULES — NEVER BREAK (violations = instant failure):
1. NEVER invent skills, technologies, employers, dates, or achievements not in the original resume
2. You MAY improve wording, reorder content, and reframe existing facts for maximum impact
3. ONLY add keywords from the job ad when they can be embedded naturally into EXISTING true statements
4. Every bullet point must refer to work the candidate actually did
5. NEVER fabricate numbers - only use metrics if they're in the original or can be reasonably inferred
6. NEVER add technologies the candidate hasn't used

ATS OPTIMIZATION RULES (CRITICAL - 40% of success):

**Section Headers (must be EXACTLY these):**
- "Professional Summary" (NOT "About Me", "Profile", "Objective")
- "Work Experience" (NOT "Employment History", "Career", "Experience")
- "Education" (NOT "Academic Background", "Qualifications")
- "Skills" (NOT "Technical Skills", "Competencies", "Expertise")
- "Certifications" (if applicable)
- "Projects" (if applicable)

Why: ATS systems search for these exact headers. Creative names cause parsing failures.

**Date Format (must be consistent):**
- Use: "January 2021 – March 2023" OR "Jan 2021 – Mar 2023" OR "01/2021 – 03/2023"
- NEVER mix formats in the same resume
- Always use en-dash (–) not hyphen (-) for date ranges
- Current roles: "January 2021 – Present"

**Bullet Point Rules:**
- Start with strong past-tense action verb (Architected, Engineered, Led, Optimized, Delivered)
- Max 2 lines per bullet (recruiters scan, don't read)
- Every bullet MUST have: Action + What + Technology/Tool + Measurable Result
- Example: "Architected **microservices** platform using **Kubernetes** and **Docker**, reducing deployment time by 60%"

**Skills Section Format:**
\`\`\`
Languages: Python, JavaScript, TypeScript, Java
Frameworks: React, Node.js, Django, Spring Boot
Tools: Docker, Kubernetes, Jenkins, Git
Platforms: AWS, Azure, GCP
Methodologies: Agile, Scrum, CI/CD, TDD
\`\`\`
Why: Grouped format helps ATS categorize skills correctly.

**ATS-Killer Formatting (NEVER USE):**
✗ Tables for layout (ATS scrambles table content)
✗ Multi-column layout (ATS reads left-to-right, merges columns)
✗ Text boxes (ATS ignores them)
✗ Headers/footers (ATS skips them)
✗ Images, logos, graphics (invisible to ATS)
✗ Special characters: ★, ●, →, ✓, ✘ (causes parsing errors)
✗ Underlining (use bold instead)
✗ Creative fonts (use Arial, Calibri, Times New Roman)

**Contact Information (top of page, NOT in header):**
\`\`\`
[Full Name]
[Email] | [Phone] | [City, State/Country] | [LinkedIn URL]
\`\`\`

**Single Column Layout:**
Everything must flow top-to-bottom in a single column. ATS reads sequentially.

BULLET QUALITY — CAR FORMAT mandatory (Context → Action → Result):

**WEAK Examples (what NOT to do):**
✗ "Responsible for backend API development"
✗ "Worked on React projects"
✗ "Helped improve system performance"
✗ "Assisted with database optimization"
✗ "Familiar with AWS services"

Why weak: Passive voice, no metrics, vague, no specific technologies.

**STRONG Examples (what to do):**
✓ "Architected **REST API** serving 200k daily requests, reducing response time 45% via **Redis** caching"
✓ "Built **React** and **TypeScript** SPA with **Redux** state management, improving load time from 3.2s to 0.8s"
✓ "Led migration of monolith to **microservices** using **Docker** and **Kubernetes**, reducing deployment time by 75%"
✓ "Optimized **PostgreSQL** queries and implemented **connection pooling**, handling 10x traffic spike with zero downtime"
✓ "Engineered **CI/CD pipeline** with **Jenkins** and **GitHub Actions**, automating deployments for 15 services"

Formula: [Action Verb] + [What you built/did] + [Technology used (bolded)] + [Measurable impact]

**Keyword Naturalization (critical for ATS):**

Instead of: "Built frontend applications"
Write: "Built scalable **React** and **TypeScript** frontend applications integrated with **REST APIs**"

Instead of: "Worked on cloud infrastructure"
Write: "Designed and deployed **AWS** infrastructure using **Terraform**, **EC2**, **S3**, and **RDS**"

Instead of: "Improved system performance"
Write: "Optimized **Node.js** backend with **Redis** caching and **database indexing**, reducing API latency by 60%"

The keywords must be woven into the natural sentence — not tacked on.

**Quantification Rules:**
- Always include numbers when possible: percentages, time saved, users served, revenue impact
- If original resume has vague scale, infer reasonable metrics: "team" → "team of 5", "users" → "10k+ users"
- Use ranges if exact numbers unknown: "50-100k users", "$1M-$5M revenue"
- Common metrics: response time, load time, uptime, throughput, cost savings, revenue, user growth, team size

MODE: ${MODES[mode].label}
${modeInstr}

ATS KEYWORD STRATEGY (CRITICAL):

**Keyword Placement Priority (weighted by ATS importance):**
1. **Skills Section (40% weight)**: List all relevant technologies from job ad
2. **Professional Summary (25% weight)**: Include top 3-5 keywords naturally
3. **Work Experience (25% weight)**: Embed keywords in bullet points with context
4. **Section Headers (10% weight)**: Use standard headers (ATS scans these first)

**Keyword Density Rules:**
- Critical keywords: appear 2-3 times (Skills + Summary + Experience)
- Secondary keywords: appear 1-2 times
- Don't stuff: max 3 bolded keywords per bullet point
- Context matters: "5 years of **React**" > "**React**"

**ATS Scoring Factors (how your resume will be ranked):**
1. Keyword Match (35%): Exact matches from job description
2. Section Completeness (25%): All standard sections present
3. Format Compliance (20%): Single column, no tables, consistent dates
4. Experience Recency (10%): Most recent role within 6 months
5. Education Match (10%): Required degree/certification present

Your goal: Achieve 85%+ keyword match while maintaining natural, readable prose.

OUTPUT FORMAT:
Plain text with **double asterisks** for keyword emphasis (renderer converts to real bold).
Standard section headers, "•" for bullets.
No markdown other than **bold**. No explanations. Output ONLY the resume. Do NOT wrap it in XML tags.`;
}

// ─── Resume user prompt ───────────────────────────────────────────────────────

export function buildResumePrompt(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  _mode: GenerationMode,
  _tier: 'large' | 'medium' | 'small' = 'large'
): string {
  const langNote = meta.mismatch
    ? `IMPORTANT: The resume is in ${meta.resumeLanguage} but the job ad is in ${meta.jobAdLanguage}. Rewrite entirely in ${meta.targetLanguage} using job market terminology native to that market.`
    : `Write in ${meta.targetLanguage}.`;

  const emphasisBlock = buildEmphasisBlock(meta.topRequirements ?? []);
  const { block: linksBlock } = parseLinksFromResume(resume);
  const resumeBody = stripLinkBlock(resume);

  return `${linksBlock ? `${linksBlock}\n\n` : ''}<candidate_resume>
${resumeBody.slice(0, 5000)}
</candidate_resume>

<job_ad>
${jobAd.slice(0, 2500)}
</job_ad>

Every skill, job title, company, date, achievement, and responsibility in your output MUST come from <candidate_resume>.

### CONTEXT ###
Candidate: ${meta.candidateName || 'Unknown'}
Target Role: ${meta.jobTitle || 'Unknown'}
Company: ${meta.companyName || 'Unknown'}
${langNote}
${emphasisBlock}

EXAMPLE — MISSING SKILLS (follow this exactly):

Resume mentions: Python, PostgreSQL, AWS
Job ad requires: Python, Kubernetes, GCP

✅ CORRECT: Emphasize Python and cloud experience (AWS). Do NOT mention Kubernetes or GCP.
❌ WRONG: "Familiar with container orchestration and cloud platforms including GCP." — Candidate never claimed this.

### REWRITING INSTRUCTIONS (internal — do NOT output any of this) ###

Internally analyse before writing:
1. Extract the 8–10 most important requirements from the job ad
2. Map each requirement to the candidate's existing experience
3. Identify the 2–3 experience items most relevant to this role
4. Note which bullets lack quantification or strong action verbs
5. List experience to minimize (irrelevant to this role)

Rewriting rules:

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

Verify before writing:
✓ All section headers are standard ALL_CAPS words
✓ Dates are consistent throughout
✓ Every bullet starts with action verb
✓ Key job-ad technologies appear bolded and naturally integrated
✓ No tables, columns, or special chars that break ATS parsers

CRITICAL: Only use facts from the original resume.

### CANDIDATE RESUME ###

Now output ONLY the rewritten resume. Do not output analysis, phase labels, or explanations.
Use this exact structure:

Line 1: Full name (plain text only — no #, no ALL_CAPS, no markdown)
Line 2: Job title (plain text)
Line 3: City, Country | email | phone | LinkedIn | GitHub
Use the short label names from CANDIDATE PROFILE LINKS (e.g. "LinkedIn", "GitHub"). Do NOT write full URLs on this line.
(blank line)
PROFESSIONAL SUMMARY
(summary paragraph)
(blank line)
WORK EXPERIENCE
(blank line)
Role Title, Company Name (Mon Year – Mon Year)
• Bullet using CAR format with **bolded tech**
• ...
(blank line)
SKILLS
Category: Skill1, **Skill2**, Skill3
...

Start the resume now:`;
}

// ─── Cover letter system prompt ───────────────────────────────────────────────

export function buildCoverLetterSystemPrompt(
  mode: GenerationMode,
  tier: 'large' | 'medium' | 'small' = 'large'
): string {
  const modeInstr = MODES[mode].toneInstruction;

  if (tier === 'small') {
    return `You are a cover letter writer. Write a focused, specific cover letter.

Rules:
1. Total body: 200–300 words
2. Structure: 4 paragraphs — Hook (specific value for this role) → Evidence (1–2 real achievements from resume) → Fit (why this company/role) → Close (confident, not desperate)
3. Bold max 4–6 job-ad keywords using **double asterisks** where they appear naturally
4. NEVER copy phrases from the job ad verbatim as if the candidate did that work
5. NEVER claim skills or experience not in the resume
6. First sentence must NOT start with "I am excited to apply" or "I am writing to"

MODE: ${MODES[mode].label}
${modeInstr}

OUTPUT: Complete cover letter with header, salutation, 4 paragraphs, sign-off. Use **bold** for keywords. Output the letter only.`;
  }

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
[City if in resume] | [Email] | [Phone if in resume] | LinkedIn | GitHub
Use the short label names from CANDIDATE PROFILE LINKS (e.g. "LinkedIn", "GitHub"). Do NOT write full URLs on this line.
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
  _mode: GenerationMode,
  _tier: 'large' | 'medium' | 'small' = 'large'
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
  const { block: linksBlock } = parseLinksFromResume(resume);
  const resumeBody = stripLinkBlock(resume);

  return `${linksBlock ? `${linksBlock}\n\n` : ''}<candidate_resume>
${resumeBody.slice(0, 4000)}
</candidate_resume>

<job_ad>
${jobAd.slice(0, 2500)}
</job_ad>

Every factual claim about the candidate MUST be traceable to a line in <candidate_resume>. Never claim skills or experience from <job_ad> alone.

EXAMPLE — CORRECT vs INCORRECT:

Job ad: "Acme Corp is hiring a Senior Backend Engineer to scale our payments infrastructure to 1M transactions/day."
Resume: "Led migration of order service from monolith to microservices at FoodCo, reducing p99 latency by 60%."

✅ CORRECT:
"I'm applying for the Senior Backend Engineer role at Acme — scaling payments infrastructure is exactly the kind of problem I worked on at FoodCo, where I led a monolith-to-microservices migration that cut p99 latency by 60%."

❌ WRONG:
"I have experience scaling payments infrastructure to 1M transactions/day and building robust systems for high-volume financial workloads."
(Pure job ad leakage — candidate never did payments or 1M/day.)

### CONTEXT ###
Candidate: ${meta.candidateName || 'Unknown'}
Role: ${meta.jobTitle || 'this role'} at ${meta.companyName || 'this company'}
Today: ${today}
${langNote}
${emphasisBlock}

### WRITING PROCESS (internal — do NOT output any of this) ###

Think through the following privately before writing:
- Top 3 requirements in the job ad
- What this company/team actually needs to accomplish
- Which 1–2 resume achievements best prove the candidate can deliver
- Specific technologies/tools emphasized in the job ad

Verify before writing:
✓ First sentence does NOT start with "I am excited/applying/writing"
✓ Company name appears in the body
✓ At least one specific metric or achievement from the resume is referenced
✓ At least one job-ad requirement is directly addressed with a bolded keyword
✓ 200–300 words in the body
✓ Only use facts from the resume

### COMPLETE COVER LETTER ###

Output ONLY the cover letter. Do NOT wrap it in XML tags. Do NOT add any commentary before or after.
Start immediately with the candidate header:`;
}

// ─── Utilities ────────────────────────────────────────────────────────────────

export function extractPlainText(raw: string): string {
  return (
    raw
      .replace(/<think>[\s\S]*?<\/think>/gi, '') // local model thinking blocks
      .replace(/<leakage_check>[\s\S]*?<\/leakage_check>/gi, '') // legacy self-check block
      // Strip any XML wrapper tags the model might echo from the prompt
      .replace(/<\/?candidate_resume>/gi, '')
      .replace(/<\/?job_ad>/gi, '')
      .replace(/<\/?leakage_check>/gi, '') // stray unclosed tags
      .replace(/^#{1,6}\s/gm, '')
      .replace(/\*\*\*(.+?)\*\*\*/g, '**$1**') // triple → double (preserve bold)
      .replace(/\*([^*]+)\*/g, '$1') // single italic → plain
      .replace(/`(.+?)`/g, '$1')
      .replace(/```[\s\S]*?```/g, '')
      .trim()
  );
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
