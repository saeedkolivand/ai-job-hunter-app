import {
  buildReferralImprovePrompt,
  buildReferralPrompt,
  extractPlainText,
  type ReferralFormat,
} from '@ajh/prompts/generate';

import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

// Referral messages are `prose_grounded`, not plain `prose`: the prompt's own
// contract is "grounded only in the candidate's résumé — no fabrication, no
// invented shared history" (`referral.ts`), the same no-fabrication requirement
// application answers have, so this drops `presencePenalty` the same way (see
// `AiProvider::sampling_profile`).

/**
 * Draft a single manual referral message (F3a) for the SELECTED format only —
 * one LLM call per format, never all three eagerly. Streams through the shared
 * pipeline (zero per-provider code, no new IPC). The person's details are
 * user-typed (no LinkedIn fetch). `extractPlainText` strips any stray
 * markdown/thinking the model echoes; the connection-note ≤300 cap is enforced
 * in the prompt and re-checked by the UI.
 */
export async function generateReferral(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  format: ReferralFormat;
  /** Hard char cap for the body (defaults to 300 for connection notes). */
  charLimit?: number;
  model: string;
  /** Message language so it streams in the right locale (default 'en'). */
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    personName,
    personRole,
    companyName,
    jobTitle,
    resume,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralPrompt(
    { personName, personRole, companyName, jobTitle, resume, format, charLimit },
    profile
  );
  const raw = await streamGenerate(model, system, user, 'referral', 'prose_grounded', {
    onToken,
    locale,
    signal,
  });
  return extractPlainText(raw);
}

/**
 * Revise an existing referral draft per a user instruction (F3a improve). Mirrors
 * {@link generateReferral} in every way but uses {@link buildReferralImprovePrompt}
 * so the revision preserves the same honesty + résumé-grounding contract, channel
 * shape, and the ≤300 hard cap for connection notes.
 *
 * SECURITY: `instruction` MUST be user-originated. Never pass scraped job-ad text,
 * company-research briefs, or any untrusted source as the instruction — it is
 * treated as a live directive by the model. The draft and résumé are fenced.
 */
export async function generateReferralImprove(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  draft: string;
  instruction: string;
  format: ReferralFormat;
  charLimit?: number;
  model: string;
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    personName,
    personRole,
    companyName,
    jobTitle,
    resume,
    draft,
    instruction,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralImprovePrompt(
    {
      personName,
      personRole,
      companyName,
      jobTitle,
      resume,
      draft,
      instruction,
      format,
      charLimit,
    },
    profile
  );
  const raw = await streamGenerate(model, system, user, 'referral', 'prose_grounded', {
    onToken,
    locale,
    signal,
  });
  return extractPlainText(raw);
}
