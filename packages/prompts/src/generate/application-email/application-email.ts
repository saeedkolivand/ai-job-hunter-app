/**
 * Application-email generation — system + user prompt for a short, professional
 * job-application email the candidate sends directly to an employer contact.
 *
 * OUTPUT CONTRACT (renderer splits on line 1 — never relax this):
 *   Line 1: Subject: <subject line>
 *   Line 2: (blank)
 *   Line 3+: email body starting with the greeting
 *
 * Provider-aware via {@link resolveProfile} (adapts across ollama / cloud / cli
 * with zero per-provider code). Locale-driven: respects
 * {@link GenerationMeta.targetLanguage}. No-fabrication: same résumé-grounded
 * honesty contract as cover-letter and referral generation.
 */

import { truncateResume } from '../../context-manager/index.js';
import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import {
  buildCompanyResearchBlock,
  buildGroundingBlock,
  buildJobAdBlock,
  buildResumeVoiceDirective,
  buildStyleReferenceBlock,
} from '../emphasis/index.js';
import { parseLinksFromResume, stripLinkBlock } from '../links/index.js';
import type { GenerationMeta } from '../modes/index.js';
import { antiAiTellProse, HUMANIZE_PROSE } from '../natural-voice/index.js';

export interface ApplicationEmailParams {
  /** Candidate's résumé text — the sole source of factual claims. */
  resume: string;
  /** The job advertisement text. */
  jobAd: string;
  /** Metadata extracted from résumé + job ad (candidate name, job title, company, locale…). */
  meta: GenerationMeta;
  /**
   * Recipient's name. When provided the greeting is "Dear {recipientName},";
   * when absent falls back to "Dear Hiring Manager,".
   */
  recipientName?: string;
  /**
   * Recipient's email address — for caller context only. The email is sent by
   * the client, not by the model; this value is NEVER echoed into the prompt or
   * the generated output.
   */
  recipientEmail?: string;
  /**
   * Optional company-research brief (same contract as cover-letter). Fenced as
   * untrusted reference material — the model uses it for "why this company"
   * context but must ignore any instructions inside it.
   */
  companyBrief?: string;
  /** Optional writing-style reference (the candidate's own writing, e.g. their
   *  résumé text) — fenced via {@link buildStyleReferenceBlock}; absent/blank
   *  renders nothing. */
  styleReference?: string;
}

/**
 * Shared no-fabrication spine. An application email that claims résumé-absent
 * skills is worse than no email — it creates a false impression the recruiter
 * will discover at interview.
 */
const EMAIL_HONESTY = `HONESTY (overrides everything else):
- Build every claim ONLY from what the résumé actually shows. Never claim, imply, or hint at skills, tools, domains, or experience the résumé does not support.
- Never inflate scope, seniority, years, team size, or outcomes beyond what the résumé states.
- If the job requires something the résumé does not show, leave it out. Never volunteer a gap or fabricate a bridge.`;

/**
 * The output-contract instruction. Stated in both system and user prompts so
 * the renderer's line-1 split for Subject vs body is always reliable.
 */
const OUTPUT_CONTRACT = `OUTPUT CONTRACT (the renderer splits on line 1 to separate subject from body — never break this):
Line 1 MUST start with exactly "Subject: " followed by the subject text. Nothing may appear before it.
Line 2 MUST be blank.
Line 3+ is the email body.`;

/**
 * Sanitize a recipient name before it is interpolated into the prompt greeting.
 *
 * Removes control characters (including bare newlines / carriage returns) so a
 * crafted multi-line "name" cannot inject prompt instructions. Collapses
 * internal whitespace to a single space, trims edges, and caps at 80 chars.
 * Returns an empty string when the result is blank, which triggers the
 * "Dear Hiring Manager," fallback in the caller.
 */
function sanitizeRecipientName(raw: string): string {
  return raw
    .replace(/[\p{Cc}]+/gu, ' ') // fold control chars + newlines into a space
    .replace(/\s+/g, ' ') // collapse consecutive spaces
    .trim()
    .slice(0, 80);
}

/**
 * Build the application-email system + user prompt.
 *
 * Returns `{ system, user }` — the same shape as every structured builder in
 * this package (referral, application-answers). Pass both to the generation
 * layer; the caller assembles the messages array.
 */
export function buildApplicationEmailPrompt(
  params: ApplicationEmailParams,
  target: PromptTarget = 'large'
): { system: string; user: string } {
  // recipientEmail is accepted in params for caller convenience but intentionally
  // never interpolated into the prompt — the email is sent by the client.
  const { resume, jobAd, meta, companyBrief = '', styleReference } = params;

  const recipientName =
    params.recipientName != null ? sanitizeRecipientName(params.recipientName) : undefined;
  const candidateName = meta.candidateName?.trim() || 'Unknown';
  const jobTitle = meta.jobTitle?.trim() || 'this role';
  const companyName = meta.companyName?.trim() || 'the company';
  const lang = meta.targetLanguage || 'en';

  const { depth, truncation, jobAdChars } = resolveProfile(target);

  const { block: linksBlock } = parseLinksFromResume(resume);
  const resumeBody = truncateResume(stripLinkBlock(resume), truncation);

  // Greeting: named recipient → "Dear {name},"; unknown → "Dear Hiring Manager,".
  const greeting = recipientName ? `Dear ${recipientName},` : 'Dear Hiring Manager,';

  const langNote = meta.mismatch
    ? `Write entirely in ${lang}. Use native phrasing and professional conventions for that market. Do NOT translate literally.`
    : `Write in ${lang}.`;

  const groundingBlock = buildGroundingBlock(resumeBody, meta.topRequirements ?? []);
  const styleBlock = buildStyleReferenceBlock(styleReference) || buildResumeVoiceDirective();

  // ── Format skeleton (shared across all depths) ────────────────────────────
  const formatSkeleton = `FORMAT:
Subject: [concise, specific subject — role name + "Application" or similar; no "RE:" or filler]

${greeting}

[Body: 2 to 3 short paragraphs, ~120 to 200 words total. NOT a cover letter — email-length only.]
- Opening paragraph: one specific, résumé-backed reason the candidate fits this role. Do not start with "I am excited to apply" or "I am writing to express my interest". Name the role and company naturally.
- Middle: one or two concrete achievements from the résumé that prove the fit — shown as sentences, not bullets.
- Closing: a brief, confident invitation to discuss further and a natural reference to the attached résumé/CV.

[Sign-off appropriate for ${lang}]
${candidateName}
[Contact line: email | phone if in résumé | key profile links — one line, concise. Use short label names from CANDIDATE PROFILE LINKS, e.g. "LinkedIn", "GitHub".]

Output the email ONLY. No commentary before or after.`;

  // ── System prompt — depth-aware ─────────────────────────────────────────────

  let system: string;

  if (depth === 'brief') {
    system = `You write short, professional job-application emails. Concise and honest — a real candidate writing, not a template.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

${langNote}

${formatSkeleton}`;
  } else if (depth === 'task') {
    system = `You are a job-application email agent. Draft a short, professional email the candidate will send directly to apply for the role. Plan → draft → verify against the acceptance checks → revise → output.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

ACCEPTANCE CHECKS (verify and revise until all pass):
- Line 1 is exactly "Subject: …" — nothing before it.
- Line 2 is blank.
- The greeting is: ${greeting}
- Every claim about the candidate is backed by the résumé; nothing from the job ad is presented as the candidate's own experience.
- Body is 120 to 200 words; reads like a person, not a template; references attaching the résumé/CV.
- Sign-off includes at least ${candidateName}'s name.
- Written entirely in ${lang}.

${langNote}

${formatSkeleton}`;
  } else {
    // full (cloud)
    system = `You write short, professional job-application emails. The kind a thoughtful candidate actually sends: concise, warm, grounded in real experience, and clearly about THIS role at THIS company — not a templated blast.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

VOICE:
${antiAiTellProse(lang)}
${HUMANIZE_PROSE}
- Conversational-professional: the candidate talking to a person, not reciting a spec. Email-length (2 to 3 paragraphs, ~120 to 200 words) — NOT a cover letter.
- Lead with a genuine, résumé-backed fit reason. Never "I am excited to apply" or "I am writing to express my interest".
- Reference attaching the résumé/CV naturally in the closing paragraph.
- Sign off from ${candidateName} with their contact line (from CANDIDATE PROFILE LINKS, if provided).

${langNote}

${formatSkeleton}`;
  }

  // ── User prompt ──────────────────────────────────────────────────────────────

  const user = `${linksBlock ? `${linksBlock}\n\n` : ''}<candidate_resume>
${resumeBody}
</candidate_resume>

${buildJobAdBlock(jobAd, jobAdChars)}
${buildCompanyResearchBlock(companyBrief)}${styleBlock}
Every factual claim about the candidate MUST be traceable to a line in <candidate_resume>. Never claim skills or experience from <job_ad> alone.

### CONTEXT ###
Candidate: ${candidateName}
Role: ${jobTitle} at ${companyName}
Greeting: ${greeting}
${langNote}
${groundingBlock ? `\n${groundingBlock}\n` : ''}
### APPLICATION EMAIL ###

Output ONLY the email. Line 1 MUST be "Subject: …". Start immediately:`;

  return { system, user };
}
