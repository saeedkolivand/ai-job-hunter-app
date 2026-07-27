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
 * {@link GenerationMeta.targetLanguage} and the resolved letter market
 * ({@link ApplicationEmailParams.market} → {@link letterConventions}) for the
 * greeting and sign-off. No-fabrication: same résumé-grounded honesty contract
 * as cover-letter and referral generation.
 */

import { truncateResume } from '../../context-manager/index.js';
import { letterConventions } from '../../locale/index.js';
import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import {
  buildCompanyResearchBlock,
  buildGroundingBlock,
  buildJobAdBlock,
  buildResumeVoiceDirective,
  buildStyleReferenceBlock,
} from '../emphasis/index.js';
import { stripLinkBlock } from '../links/index.js';
import type { GenerationMeta } from '../modes/index.js';
import {
  antiAiTellProse,
  HUMANIZE_PROSE,
  type OutputTone,
  toneDirective,
} from '../natural-voice/index.js';

export interface ApplicationEmailParams {
  /** Candidate's résumé text — the sole source of factual claims. */
  resume: string;
  /** The job advertisement text. */
  jobAd: string;
  /** Metadata extracted from résumé + job ad (candidate name, job title, company, locale…). */
  meta: GenerationMeta;
  /**
   * Recipient's name. When provided the greeting uses the market's *named*
   * salutation with this name substituted; when absent (or blank after
   * sanitizing) the market's generic salutation is used instead — e.g. a DACH
   * market yields "Sehr geehrte Damen und Herren,", never an English default.
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
  /**
   * Resolved letter-market id (see `resolveMarket`) — drives the greeting and
   * sign-off etiquette exactly like the cover letter's `<market_conventions>`.
   * Absent/unknown falls back to the international baseline ("Dear Hiring
   * Manager,").
   */
  market?: string;
  /**
   * User's Output Tone preference (Settings → Output Tone). Shapes register and
   * word choice only — never the honesty contract or the market conventions.
   * Defaults to `professional` (see {@link toneDirective}).
   */
  tone?: OutputTone;
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
 * Returns an empty string when the result is blank, which makes the caller fall
 * back to the market's generic salutation.
 */
function sanitizeRecipientName(raw: string): string {
  return raw
    .replace(/[\p{Cc}]+/gu, ' ') // fold control chars + newlines into a space
    .replace(/\s+/g, ' ') // collapse consecutive spaces
    .trim()
    .slice(0, 80);
}

/**
 * Render a market's *named* salutation pattern with the sanitized recipient
 * name. Convention patterns carry placeholders (`{title}`, `{firstName}`,
 * `{lastName}`, `{patronymic}`) but a caller only ever knows one free-text
 * name, so the name slot ({lastName}, else {firstName}) takes it — every
 * occurrence, since gendered patterns repeat it — and the remaining
 * placeholders are dropped, then stray whitespace is collapsed.
 * Patterns without any placeholder (e.g. `fr`: "Madame, / Monsieur,") are
 * returned as-is: that market does not name the recipient in the salutation.
 */
function renderNamedSalutation(pattern: string, name: string): string {
  const slot = pattern.includes('{lastName}')
    ? '{lastName}'
    : pattern.includes('{firstName}')
      ? '{firstName}'
      : '';
  return (slot ? pattern.split(slot).join(name) : pattern)
    .replace(/\{[A-Za-z]+\}/g, '') // drop placeholders we cannot fill
    .replace(/\s+/g, ' ')
    .replace(/\s+([,.:;!?，、。：！])/g, '$1')
    .trim();
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
  const { resume, jobAd, meta, companyBrief = '', styleReference, market, tone } = params;

  const recipientName =
    params.recipientName != null ? sanitizeRecipientName(params.recipientName) : undefined;
  const candidateName = meta.candidateName?.trim() || 'Unknown';
  const jobTitle = meta.jobTitle?.trim() || 'this role';
  const companyName = meta.companyName?.trim();
  const hasCompany = !!meta.companyName?.trim();
  const lang = meta.targetLanguage || 'en';

  const { depth, truncation, jobAdChars } = resolveProfile(target);

  const resumeBody = truncateResume(stripLinkBlock(resume), truncation);

  // ── Greeting + sign-off: market etiquette, never an English default ─────────
  // Same rule as the cover letter's <market_conventions> (see
  // `buildMarketConventionsBlock`): the market's own salutation/sign-off are used
  // verbatim when the email language matches that market's native language;
  // otherwise the model writes the formal equivalent in the email language while
  // keeping the market's register. ONE `greeting` string feeds all three
  // injection points (FORMAT skeleton, task-depth acceptance check, user
  // CONTEXT) so they can never disagree.
  const c = letterConventions(market);
  const sameLanguage = c.nativeLanguage === lang.slice(0, 2).toLowerCase();
  const salutation = recipientName
    ? renderNamedSalutation(c.salutations.named, recipientName)
    : c.salutations.generic;
  // Gendered / either-or patterns ("… Frau X, / … Herr X,", "Dear Mr/Ms X") list
  // alternatives that a free-text name cannot resolve — the writer picks one and
  // trims the name to the form that market uses (DACH addresses by surname).
  // Markets whose pattern has no alternatives (the en/intl baseline) get no such
  // clause, so their greeting stays exactly what it has always been.
  const pickOne = salutation.includes('/')
    ? " Write exactly one variant — the one that fits the recipient, in this market's form of address (surname only where that is the convention) — never the alternatives together."
    : '';
  const greeting = sameLanguage
    ? `"${salutation}"${pickOne}`
    : `the formal ${lang} equivalent of "${salutation}", at this market's level of formality (never a casual greeting).${pickOne}`;
  const signoff = sameLanguage
    ? `"${c.signoffs[0]}"`
    : `the formal ${lang} equivalent of "${c.signoffs[0]}"`;

  const langNote = meta.mismatch
    ? `Write entirely in ${lang}. Use native phrasing and professional conventions for that market. Do NOT translate literally.`
    : `Write in ${lang}.`;

  const groundingBlock = buildGroundingBlock(resumeBody, meta.topRequirements ?? []);
  const styleBlock = buildStyleReferenceBlock(styleReference) || buildResumeVoiceDirective();
  // Output Tone (Settings) — same composition rule as the cover letter: register
  // and word choice only, never overriding honesty or the market conventions.
  const toneLine = toneDirective(tone);

  // ── Format skeleton (shared across all depths) ────────────────────────────
  const formatSkeleton = `FORMAT:
Subject: [concise, specific subject — role name + "Application" or similar; no "RE:" or filler]

[Greeting: ${greeting}]

[Body: 2 to 3 short paragraphs, ~120 to 200 words total. NOT a cover letter — email-length only.]
- Opening paragraph: one specific, résumé-backed reason the candidate fits this role. Do not start with "I am excited to apply" or "I am writing to express my interest". Name the role and company naturally.${hasCompany ? '' : ' (company name unknown: name only the role, never invent, name, or write a company placeholder such as "Company" or "Unternehmen")'}
- Middle: one or two concrete achievements from the résumé that prove the fit — shown as sentences, not bullets.
- Closing: a brief, confident invitation to discuss further and a natural reference to the attached résumé/CV.

[Sign-off: ${signoff}]
${candidateName}
[Nothing after the name: no contact line, email address, phone number, postal address, or profile links.]

Output the email ONLY. No commentary before or after.`;

  // ── System prompt — depth-aware ─────────────────────────────────────────────

  let system: string;

  if (depth === 'brief') {
    system = `You write short, professional job-application emails. Concise and honest — a real candidate writing, not a template.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

${toneLine}

${langNote}

${formatSkeleton}`;
  } else if (depth === 'task') {
    system = `You are a job-application email agent. Draft a short, professional email the candidate will send directly to apply for the role. Plan → draft → verify against the acceptance checks → revise → output.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

ACCEPTANCE CHECKS (verify and revise until all pass):
- Line 1 is exactly "Subject: …" — nothing before it.
- Line 2 is blank.
- The greeting follows: ${greeting}
- Every claim about the candidate is backed by the résumé; nothing from the job ad is presented as the candidate's own experience.
- Body is 120 to 200 words; reads like a person, not a template; references attaching the résumé/CV.
- Sign-off is ${signoff} followed by ${candidateName}'s name, and nothing after it (no contact line, email, phone, or links).
- Written entirely in ${lang}.

${toneLine}

${langNote}

${formatSkeleton}`;
  } else {
    // full (cloud)
    system = `You write short, professional job-application emails. The kind a thoughtful candidate actually sends: concise, warm, grounded in real experience, and clearly ${hasCompany ? 'about THIS role at THIS company' : 'about THIS role'} — not a templated blast.

${OUTPUT_CONTRACT}

${EMAIL_HONESTY}

VOICE:
${antiAiTellProse(lang)}
${HUMANIZE_PROSE}
- Conversational-professional: the candidate talking to a person, not reciting a spec. Email-length (2 to 3 paragraphs, ~120 to 200 words) — NOT a cover letter.
- Lead with a genuine, résumé-backed fit reason. Never "I am excited to apply" or "I am writing to express my interest".
- Reference attaching the résumé/CV naturally in the closing paragraph.
- Sign off from ${candidateName} — the name alone, with no contact line, email address, phone number, or profile links after it.

${toneLine}

${langNote}

${formatSkeleton}`;
  }

  // ── User prompt ──────────────────────────────────────────────────────────────

  const user = `<candidate_resume>
${resumeBody}
</candidate_resume>

${buildJobAdBlock(jobAd, jobAdChars)}
${buildCompanyResearchBlock(companyBrief)}${styleBlock}
Every factual claim about the candidate MUST be traceable to a line in <candidate_resume>. Never claim skills or experience from <job_ad> alone.

### CONTEXT ###
Candidate: ${candidateName}
Role: ${hasCompany ? `${jobTitle} at ${companyName}` : jobTitle}
Greeting: ${greeting}
${langNote}
${groundingBlock ? `\n${groundingBlock}\n` : ''}
### APPLICATION EMAIL ###

Output ONLY the email. Line 1 MUST be "Subject: …". Start immediately:`;

  return { system, user };
}
