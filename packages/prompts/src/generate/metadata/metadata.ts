/** Metadata-extraction prompt + validator. */

import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import { buildJobAdBlock } from '../emphasis/index.js';
import { parseLinksFromResume, stripLinkBlock } from '../links/index.js';
import type { GenerationMeta } from '../modes/index.js';

export function buildMetadataPrompt(
  resume: string,
  jobAd: string,
  target: PromptTarget = 'large'
): { system: string; user: string } {
  // One-shot example for brief (small / unknown-local) targets — boosts JSON compliance.
  const oneShot =
    resolveProfile(target).depth === 'brief'
      ? `\nExample output:\n{"candidateName":"Jane Smith","jobTitle":"Senior Frontend Engineer","companyName":"Acme Corp","resumeLanguage":"en","jobAdLanguage":"en","topRequirements":["React","TypeScript","GraphQL"],"candidateSeniority":"senior","jobLocation":"Berlin, Germany (hybrid)","jobCountry":"DE"}\n`
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

${buildJobAdBlock(jobAd, 4000)}

Return this exact JSON (no other text):
{
  "candidateName": "full name from resume or empty string",
  "jobTitle": "exact job title from job ad",
  "companyName": "company name from job ad or empty string",
  "resumeLanguage": "ISO 639-1 code e.g. en, de, fr",
  "jobAdLanguage": "ISO 639-1 code e.g. en, de, fr",
  "topRequirements": ["up to 12 exact technology names and skills from the job ad that should be bolded — prefer specific names like React, TypeScript, AWS, Kubernetes over generic terms like communication or teamwork"],
  "candidateSeniority": "junior|mid|senior|lead|executive",
  "jobLocation": "the job's location exactly as written in the ad (city/country/remote), or empty string if not stated",
  "jobCountry": "the ISO-3166 alpha-2 country code of where the job is based, e.g. DE, US, GB, FR — infer from the location/company; empty string if truly unknown"
}
${oneShot}
Return ONLY the JSON object.`,
  };
}

/** A model-supplied language code, coerced to the `'en'` default. `??` alone is
 *  not enough: the model routinely answers with an EMPTY STRING rather than
 *  omitting the key, and `'' ?? 'en'` is `''`. */
function toLanguage(v: unknown): string {
  return typeof v === 'string' && v.trim() !== '' ? v.trim() : 'en';
}

/** Whether a raw (pre-`toLanguage`) language field was blank/missing.
 *  `toLanguage()` defaults a blank value to `'en'` for downstream prompting,
 *  which would otherwise masquerade as a genuine "en" detection and falsely
 *  trip the mismatch guard below (e.g. resume "de" vs a blank jobAd reading
 *  as "de" vs "en"). */
function isBlank(v: unknown): boolean {
  return typeof v !== 'string' || v.trim() === '';
}

export function validateMetadata(raw: string): GenerationMeta | null {
  try {
    const jsonStr = raw.slice(raw.indexOf('{'), raw.lastIndexOf('}') + 1);
    const parsed = JSON.parse(jsonStr);
    // Coerce FIRST, then decide `mismatch` from the coerced values — the same
    // order `analyze/validate.ts` and `@ajh/shared`'s `detectLanguages` use.
    const resumeLanguage = toLanguage(parsed.resumeLanguage);
    const jobAdLanguage = toLanguage(parsed.jobAdLanguage);
    return {
      candidateName: parsed.candidateName ?? '',
      jobTitle: parsed.jobTitle ?? '',
      companyName: parsed.companyName ?? '',
      resumeLanguage,
      jobAdLanguage,
      // Only a mismatch when BOTH sides are known and actually differ. Without
      // the `'unknown'` guard, an undetected side raised a spurious
      // "rewrite entirely / do not translate" instruction in the prompt. The
      // blank check must run against the RAW value — `toLanguage()` already
      // defaulted a blank side to 'en' above, so checking the normalized
      // value here would miss e.g. resume "de" vs a blank jobAd.
      mismatch:
        !isBlank(parsed.resumeLanguage) &&
        !isBlank(parsed.jobAdLanguage) &&
        resumeLanguage !== 'unknown' &&
        jobAdLanguage !== 'unknown' &&
        resumeLanguage !== jobAdLanguage,
      targetLanguage: jobAdLanguage,
      topRequirements: Array.isArray(parsed.topRequirements) ? parsed.topRequirements : [],
      jobLocation: typeof parsed.jobLocation === 'string' ? parsed.jobLocation : '',
      // Normalize to an upper-case 2-letter code; drop anything that isn't one.
      jobCountry:
        typeof parsed.jobCountry === 'string' && /^[A-Za-z]{2}$/.test(parsed.jobCountry.trim())
          ? parsed.jobCountry.trim().toUpperCase()
          : '',
    };
  } catch {
    return null;
  }
}
