/**
 * Resume Builder synthesis prompt (#1 / phase B9).
 *
 * The builder has NO base résumé and NO job ad — the candidate's structured
 * interview answers ARE the grounding source. So this is a from-scratch writer
 * with the same hard no-fabrication contract as the tailoring path, but the
 * grounding tag is `<interview_answers>` rather than `<candidate_resume>`. The
 * output uses the same markdown grammar and localized section headers as
 * `generate/resume.ts`, so the existing export / preview / link handling apply
 * unchanged.
 */

import { buildEmphasisDirectivesBlock } from '../generate/emphasis/index.js';
import type { GenerationMeta } from '../generate/modes/index.js';
import { resumeConventions } from '../locale/index.js';
import { type PromptTarget, resolveProfile } from '../provider/index.js';
import type {
  InterviewAnswers,
  InterviewEducation,
  InterviewEntry,
  InterviewExperience,
  InterviewProject,
  InterviewPublication,
} from './types.js';

const HARD_RULES = `NEVER BREAK THESE RULES:
1. Use ONLY facts the candidate provided in <interview_answers>. NEVER invent employers, job titles, dates, metrics, skills, schools, degrees, or achievements.
2. NEVER add a technology, tool, or number the candidate did not state. If a bullet has no number, keep it qualitative — do NOT fabricate one.
3. You MAY reword, reorder, group, and reframe the provided facts for impact and ATS — but every claim must trace to an answer.
4. Keep EVERY work role and education entry the candidate gave — reorder/condense the bullets within a role, never drop a role or entry.
5. Keep every link the candidate provided inline on its item as [label](url) — never drop a link.`;

const SUMMARY_RULE = `PROFESSIONAL SUMMARY:
- If the candidate wrote a summary, keep its substance and claims (you may lightly polish grammar and flow) — do NOT replace it with a generic one.
- If they wrote none, write a 2–3 sentence summary derived strictly from the answers (seniority and domain inferred from their roles) — invent no years, metrics, or claims.`;

function buildBuilderSystemFull(): string {
  return `You are an expert résumé writer with deep knowledge of ATS systems, recruiter behavior, and modern hiring practices. Build a complete, ATS-ready résumé from the candidate's interview answers.

${HARD_RULES}

${SUMMARY_RULE}

SECTION HEADERS: use the target market's standard headers exactly as the task provides them, consistently. Never invent creative section names — ATS parsers rely on standard headers. Include optional sections (Projects, Publications, Certifications, etc.) only when the answers contain them.

DATE FORMAT: one consistent format throughout (the task gives an example). Use an en-dash (–) for ranges and the target language's word for "Present" for current roles.

BULLET POINTS:
- Start each with a strong action verb (Architected, Led, Optimized, Delivered…).
- Action + What + Technology/Tool + Measurable result WHEN the candidate supplied a number.
- Max ~2 lines per bullet. Wrap genuine job-relevant keywords in **double asterisks** (max 2–3 per bullet).

SKILLS: group ATS-style (e.g. Languages / Frameworks / Tools / Platforms) when the entries support it; otherwise a single clean line.

OUTPUT: plain text only. Start with the candidate's name, then a contact line (these are replaced by the saved contact profile on export). Standard localized section headers, "•" for bullets, **double asterisks** for emphasis, no other markdown, no commentary, no XML tags. Output ONLY the résumé.`;
}

function buildBuilderSystemBrief(): string {
  return `You are an expert résumé writer. Build a complete, ATS-ready résumé from the candidate's interview answers.

${HARD_RULES}

${SUMMARY_RULE}

REQUIRED SECTION HEADERS: use the localized headers the task provides, consistently. Add optional sections (Projects, Publications, Certifications) only when the answers include them.

DATE FORMAT: one consistent format (the task gives an example); en-dash for ranges; the target language's word for "Present" for current roles.

Every bullet: strong action verb + what + technology + a measurable result only if the candidate gave one. Wrap key terms in **double asterisks** (max 2–3 per bullet).

OUTPUT: plain text. Start with the name + a contact line (replaced by the saved profile on export). Standard headers, "•" bullets, only **bold** markdown. Output ONLY the résumé.`;
}

function buildBuilderSystemTask(): string {
  return `You are a résumé-writing agent working a TASK. You may plan, draft, self-review, and revise before finalizing.

GOAL: build a complete, ATS-ready résumé from the candidate's interview answers, in the target language, ready to pass ATS and impress a recruiter.

${HARD_RULES}

${SUMMARY_RULE}

ACCEPTANCE CHECKS — verify and revise until all pass:
- Output is the finished résumé only (no commentary), in the target language, using that market's standard section headers and one consistent date format.
- No skill, employer, date, or number appears that the candidate did not provide.
- Every provided link is preserved inline on its item.
- Optional sections appear only when the answers contain them.

OUTPUT: the finished résumé (plain text; name + contact line first — replaced by the saved profile on export; "•" bullets; only **bold** markdown).`;
}

export function buildBuilderSystemPrompt(target: PromptTarget = 'large'): string {
  const { depth } = resolveProfile(target);
  if (depth === 'task') return buildBuilderSystemTask();
  if (depth === 'brief') return buildBuilderSystemBrief();
  return buildBuilderSystemFull();
}

// ─── Answer rendering ────────────────────────────────────────────────────────

function linkSuffix(label: string | undefined, link: string | undefined): string {
  if (!link?.trim()) return '';
  return ` [${(label || 'link').trim()}](${link.trim()})`;
}

function renderExperience(items: InterviewExperience[]): string {
  return items
    .filter((e) => e.title?.trim() || e.company?.trim())
    .map((e) => {
      const loc = e.location?.trim() ? ` (${e.location.trim()})` : '';
      const end = e.current ? 'Present' : e.endDate?.trim() || 'Present';
      const dates = e.startDate?.trim() ? ` | ${e.startDate.trim()} – ${end}` : '';
      const head = `- ${e.title?.trim() || 'Role'} @ ${e.company?.trim() || 'Company'}${loc}${dates}`;
      const bullets = (e.bullets ?? [])
        .map((b) => b.trim())
        .filter(Boolean)
        .map((b) => `  • ${b}`)
        .join('\n');
      return bullets ? `${head}\n${bullets}` : head;
    })
    .join('\n');
}

function renderEducation(items: InterviewEducation[]): string {
  return items
    .filter((e) => e.degree?.trim() || e.institution?.trim())
    .map((e) => {
      const loc = e.location?.trim() ? `, ${e.location.trim()}` : '';
      const range = [e.startDate?.trim(), e.endDate?.trim()].filter(Boolean).join(' – ');
      const dates = range ? ` (${range})` : '';
      const head = `- ${e.degree?.trim() || 'Degree'} — ${e.institution?.trim() || 'Institution'}${loc}${dates}`;
      const details = e.details?.trim() ? `\n  ${e.details.trim()}` : '';
      return `${head}${details}`;
    })
    .join('\n');
}

function renderProjects(items: InterviewProject[]): string {
  return items
    .filter((p) => p.name?.trim())
    .map((p) => {
      const desc = p.description?.trim() ? ` — ${p.description.trim()}` : '';
      return `- ${p.name.trim()}${desc}${linkSuffix(p.name, p.link)}`;
    })
    .join('\n');
}

function renderPublications(items: InterviewPublication[]): string {
  return items
    .filter((p) => p.title?.trim())
    .map((p) => {
      const venue = p.venue?.trim() ? `, ${p.venue.trim()}` : '';
      const year = p.year?.trim() ? ` (${p.year.trim()})` : '';
      return `- ${p.title.trim()}${venue}${year}${linkSuffix(p.title, p.link)}`;
    })
    .join('\n');
}

function renderEntries(items: InterviewEntry[]): string {
  return items
    .filter((e) => e.title?.trim())
    .map((e) => {
      const detail = e.detail?.trim() ? ` — ${e.detail.trim()}` : '';
      const year = e.year?.trim() ? ` (${e.year.trim()})` : '';
      return `- ${e.title.trim()}${detail}${year}`;
    })
    .join('\n');
}

/** Render the answers into the `<interview_answers>` body. Empty sections are omitted. */
export function renderInterviewAnswers(answers: InterviewAnswers): string {
  const parts: string[] = [];
  if (answers.fullName?.trim()) parts.push(`NAME: ${answers.fullName.trim()}`);
  if (answers.headline?.trim()) parts.push(`HEADLINE: ${answers.headline.trim()}`);
  if (answers.summary?.trim())
    parts.push(`SUMMARY (candidate-written — keep its substance):\n${answers.summary.trim()}`);

  const exp = renderExperience(answers.experience ?? []);
  if (exp) parts.push(`EXPERIENCE:\n${exp}`);

  const edu = renderEducation(answers.education ?? []);
  if (edu) parts.push(`EDUCATION:\n${edu}`);

  const skills = (answers.skills ?? []).map((s) => s.trim()).filter(Boolean);
  if (skills.length) parts.push(`SKILLS: ${skills.join(', ')}`);

  const projects = renderProjects(answers.projects ?? []);
  if (projects) parts.push(`PROJECTS:\n${projects}`);

  const pubs = renderPublications(answers.publications ?? []);
  if (pubs) parts.push(`PUBLICATIONS:\n${pubs}`);

  const awards = renderEntries(answers.awards ?? []);
  if (awards) parts.push(`AWARDS:\n${awards}`);

  const volunteer = renderEntries(answers.volunteer ?? []);
  if (volunteer) parts.push(`VOLUNTEERING:\n${volunteer}`);

  const langs = (answers.languages ?? []).map((s) => s.trim()).filter(Boolean);
  if (langs.length) parts.push(`LANGUAGES: ${langs.join(', ')}`);

  const certs = (answers.certifications ?? []).map((s) => s.trim()).filter(Boolean);
  if (certs.length) parts.push(`CERTIFICATIONS: ${certs.join(', ')}`);

  return parts.join('\n\n');
}

export function buildInterviewResumePrompt(
  answers: InterviewAnswers,
  meta: GenerationMeta
): string {
  // Section headers + date format follow the target market's conventions.
  const conv = resumeConventions(meta.targetLanguage);
  const conventionsNote = `CONVENTIONS (target market: ${meta.targetLanguage}): use these section headers — ${conv.headers.summary} / ${conv.headers.experience} / ${conv.headers.education} / ${conv.headers.skills}; and one consistent date format like ${conv.dateExample}.`;
  const directivesBlock = buildEmphasisDirectivesBlock(meta.emphasis);
  const hasSummary = Boolean(answers.summary?.trim());

  return `<interview_answers>
${renderInterviewAnswers(answers)}
</interview_answers>

Every employer, title, date, skill, achievement, and link in your output MUST come from <interview_answers>. Add nothing the candidate did not state.

### CONTEXT ###
Candidate: ${meta.candidateName || answers.fullName || 'Unknown'}
${answers.headline?.trim() ? `Target role / headline: ${answers.headline.trim()}` : ''}
Write in ${meta.targetLanguage}.
${conventionsNote}
${directivesBlock ? `${directivesBlock}\n` : ''}
### TASK ###
Build a complete, single-column, ATS-ready résumé from the answers above.
- Lead with a ${hasSummary ? 'Professional Summary based on the candidate-written summary (keep its substance)' : 'Professional Summary of 2–3 sentences derived strictly from the answers'}.
- Then Work Experience (every role, most relevant bullets first within each role), Education, and Skills.
- Add Projects / Publications / Certifications / Awards / Volunteering / Languages sections ONLY when the answers include them, using the market's standard header names.
- Keep every provided link inline on its item as [label](url).

Output ONLY the résumé as plain text — name + contact line first (replaced by the saved contact profile on export), standard localized section headers, "•" bullets, **double asterisks** for emphasis only.`;
}
