/**
 * Job-ad summary assistant — a résumé-INDEPENDENT "key notes" digest of a single
 * job ad. Distinct from the résumé-vs-job ATS analysis (`buildAnalysisPrompt`) and
 * from the application-*answers* assistant: this one never sees a résumé and never
 * scores fit. It just distils what the ad itself says into a short, scannable
 * markdown overview, in the ad's own language.
 *
 * One prompt builder + one system prompt, zero deps, provider-aware (the JD is
 * truncated to the tier's `jobAdChars`) and locale-driven (output language follows
 * the ad). Mirrors the {@link buildApplicationAnswerPrompt} job-ad handling.
 */

import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import type { GenerationMeta } from '../modes/index.js';

/** System prompt — the no-fabrication contract for the digest. */
export function buildJobAdSummarySystemPrompt(): string {
  return `You are summarizing a single job advertisement into a short, scannable digest of its key notes.

ABSOLUTE RULES (never break these):
1. Summarize ONLY what the ad actually states in <job_ad>. NEVER fabricate, infer, or add facts (skills, salary, location, seniority, company details) that are not present in the ad.
2. If the ad does not cover one of the sections below, OMIT that section entirely — do not guess or pad.
3. Write the digest in the AD'S OWN LANGUAGE (the language the ad is written in), not in English by default.
4. Be concise: a scannable overview, NOT a re-print of the ad. Compress lists, drop boilerplate and legal disclaimers.
5. Output concise markdown only — bold section labels and short bullet points. No preamble, no closing remarks, no headings beyond the bold labels below.`;
}

/**
 * Build the user prompt for a job-ad summary. Wraps the (tier-truncated) ad text
 * in the same `<job_ad>` fence the other generators use and asks for a short
 * markdown digest. No résumé and no company brief are involved.
 */
export function buildJobAdSummaryPrompt(
  jobAd: string,
  meta?: GenerationMeta | null,
  target?: PromptTarget
): string {
  const { jobAdChars } = resolveProfile(target);

  const lang = meta?.targetLanguage;
  const langNote = lang
    ? `Write the digest in ${lang} (the ad's language).`
    : `Write the digest in the ad's own language.`;

  return `<job_ad>
${jobAd.slice(0, jobAdChars)}
</job_ad>

### TASK ###
Summarize the job ad above into a short, scannable markdown digest. ${langNote}

Use these bold section labels, in this order, and OMIT any the ad does not cover:
**Role & seniority** — the title and level the ad is for.
**Key responsibilities** — what the person will actually do.
**Must-haves** — required skills, experience, and qualifications.
**Nice-to-haves** — preferred / bonus qualifications.
**Comp & logistics** — salary, location, remote/hybrid, start date, and similar.

Keep it brief and readable — short bullet points under each label, no re-printing of the ad. Output ONLY the markdown digest:`;
}
