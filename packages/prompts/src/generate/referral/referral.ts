/**
 * Manual referral-helper generation (F3a).
 *
 * Drafts a short, honest, low-pressure message asking a specific person for a
 * referral at the company the candidate is applying to. The person's details are
 * entered MANUALLY by the user — there is NO LinkedIn scraping or profile fetch —
 * so this builder only takes the typed name/role plus the candidate's own résumé
 * and the target role.
 *
 * Three format variants, all grounded in the résumé (never fabricate) and warm
 * but professional:
 * - `connection_note` — a LinkedIn connection-request note with a HARD ≤300
 *   character limit (the platform's invite-note cap), stated as an absolute
 *   constraint in the system prompt and re-enforced in the UI.
 * - `linkedin_message` — a concise direct message (no hard cap, kept tight).
 * - `email` — a subject line plus a short body.
 *
 * Provider-aware via {@link resolveProfile} — like every other builder it accepts
 * a bare tier or a full provider profile, so it adapts across ollama / cloud /
 * cli with zero per-provider code: the resolved profile sizes how much of the
 * résumé context the prompt carries and how verbose the framing is.
 */

import { truncateResume } from '../../context-manager/index.js';
import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import { stripLinkBlock } from '../links/index.js';
import { ANTI_AI_TELL_PROSE } from '../natural-voice/index.js';

/** The outreach format the user picked — drives the variant + any hard limits. */
export type ReferralFormat = 'email' | 'linkedin_message' | 'connection_note';

export interface ReferralPromptParams {
  /** The person being asked for a referral (typed by the user). */
  personName: string;
  /** Their role/title, if the user noted it. */
  personRole?: string;
  /** The target company. */
  companyName: string;
  /** The role the candidate is applying for. */
  jobTitle: string;
  /** The candidate's own résumé — the only source of factual claims. */
  resume: string;
  /** Which message variant to write. */
  format: ReferralFormat;
  /**
   * Hard character cap for the message body. Defaults to {@link CONNECTION_NOTE_LIMIT}
   * for `connection_note`; ignored (no hard cap) for the other formats unless set.
   */
  charLimit?: number;
}

/** LinkedIn's connection-request note hard limit, in characters. */
export const CONNECTION_NOTE_LIMIT = 300;

/** Human-readable format label woven into the prompt framing. */
const FORMAT_LABELS: Record<ReferralFormat, string> = {
  email: 'a referral email',
  linkedin_message: 'a LinkedIn direct message',
  connection_note: 'a LinkedIn connection-request note',
};

/**
 * The shared honesty + tone spine for every referral variant. A referral ask is
 * a real person reaching out to another real person, so the same no-fabrication
 * contract used for résumé / cover-letter generation applies: only résumé-backed
 * claims, no invented shared history, low pressure.
 */
const REFERRAL_CONTRACT = `ABSOLUTE RULES (never break these):
1. Every factual claim about the candidate MUST be traceable to <candidate_resume>. NEVER invent skills, employers, titles, metrics, dates, projects, or a shared history with the recipient.
2. Do NOT claim the candidate and the recipient already know each other, worked together, or share a connection unless that is explicitly stated. If nothing genuine is shared, keep it a polite cold outreach.
3. One clear, low-pressure ask: would they be open to referring the candidate (or pointing them to the right person) for the role. Never demanding, never entitled, no guilt.
4. Warm, specific, human, and brief. Name the real role and company, anchor on one genuine, résumé-backed reason the candidate fits. No buzzwords, no flattery padding.
5. Write in the same language as the résumé.
6. Output the message ONLY. No preamble, no commentary, no surrounding quotation marks, no labels other than the email's "Subject:" line when the format is email.`;

/**
 * Build the referral system + user prompt. The system prompt fixes the contract
 * and the format-specific shape (and the hard ≤300 cap for a connection note);
 * the user prompt carries the grounded résumé context and the recipient + role.
 */
export function buildReferralPrompt(
  params: ReferralPromptParams,
  target: PromptTarget = 'large'
): { system: string; user: string } {
  const { personRole, resume, format } = params;
  // Scraped job fields (company/title) and the typed person name can carry stray
  // whitespace/newlines — trim at the builder boundary and use the trimmed values
  // in every interpolation.
  const personName = params.personName.trim();
  const companyName = params.companyName.trim();
  const jobTitle = params.jobTitle.trim();
  // Consume the resolved profile: section-aware résumé truncation sized to the
  // provider (whole résumé for large-context providers, bounded for small local
  // models) and prompt verbosity from the framing depth — same provider
  // abstraction as the other generation builders.
  const { depth, truncation } = resolveProfile(target);
  const resumeBody = truncateResume(stripLinkBlock(resume), truncation);

  const limit =
    format === 'connection_note' ? (params.charLimit ?? CONNECTION_NOTE_LIMIT) : params.charLimit;

  const formatRule = buildFormatRule(format, limit);
  const role = personRole?.trim();
  const recipient = role ? `${personName} (${role})` : personName;

  // brief = compact/imperative framing for small local models; full/task get the
  // fuller guidance. The contract + format rule are identical across depths so
  // the hard cap is never weakened by a smaller prompt.
  const guidance =
    depth === 'brief'
      ? `You help a job candidate write ${FORMAT_LABELS[format]} asking a specific person for a referral. Keep it short, warm, honest, and grounded only in the candidate's résumé.`
      : `You are helping a job candidate reach out to a specific person to ask for a referral. You write ${FORMAT_LABELS[format]} that is warm, specific, and honest. The kind of note a thoughtful person actually sends, never a templated mass message. Lead with one genuine, résumé-backed reason the candidate is a strong fit for the role, then make a single low-pressure ask.`;

  const system = `${guidance}

${REFERRAL_CONTRACT}

FORMAT (${FORMAT_LABELS[format]}):
${formatRule}

${ANTI_AI_TELL_PROSE}`;

  const user = `<candidate_resume>
${resumeBody}
</candidate_resume>

Every factual claim about the candidate MUST be traceable to a line in <candidate_resume>. Never invent experience, skills, or a shared history with the recipient.

### CONTEXT ###
Recipient: ${recipient}
Company: ${companyName}
Role the candidate is applying for: ${jobTitle}

Write ${FORMAT_LABELS[format]} from the candidate to ${personName}, asking, politely and low-pressure, whether they'd be open to referring the candidate for the ${jobTitle} role at ${companyName}. Ground every claim in the résumé.${
    format === 'connection_note'
      ? `\n\nHARD CONSTRAINT: the entire note MUST be ${limit} characters or fewer. Count characters and stay under the limit. A note over ${limit} characters will be rejected.`
      : ''
  }
Output ONLY the message:`;

  return { system, user };
}

/** Per-format shape + any hard limit, stated in the system prompt. */
function buildFormatRule(format: ReferralFormat, limit?: number): string {
  switch (format) {
    case 'connection_note':
      return `- This is a LinkedIn connection-request note. It has an ABSOLUTE hard limit of ${limit ?? CONNECTION_NOTE_LIMIT} characters for the ENTIRE note. This is a platform limit, not a style preference. Going over is a hard failure: count characters and keep the whole note at or under ${limit ?? CONNECTION_NOTE_LIMIT} characters.
- One or two short sentences: who the candidate is in a few words, the genuine fit, and the single ask. No greeting line and no sign-off block (there is no room).`;
    case 'linkedin_message':
      return `- A concise LinkedIn direct message. No hard character cap, but keep it tight, roughly 3 to 5 short sentences a busy person reads in one glance.
- A brief greeting by first name, one genuine résumé-backed reason for the fit, the single low-pressure ask, and a short thank-you.`;
    case 'email':
    default:
      return `- A short referral email. Start with a "Subject:" line on its own (concise and specific), then a blank line, then the body.
- Body: a brief greeting by name, one or two sentences on the genuine résumé-backed fit, the single low-pressure ask, and a short sign-off. Keep the whole email well under a screen.`;
  }
}
