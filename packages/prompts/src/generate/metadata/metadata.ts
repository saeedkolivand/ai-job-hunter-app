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

/** Job-ad section headings that a naive extractor (LLM or regex) returns as if
 *  they were the employer. Normalized to lowercase with apostrophes stripped so
 *  `You'll Do` / `You´ll Do` / `you ll do` all match one entry. */
const HEADING_DENYLIST = new Set([
  'about us',
  'about the company',
  'about the role',
  'apply now',
  'benefits',
  'jetzt bewerben',
  'job description',
  'our company',
  'requirements',
  'responsibilities',
  'the company',
  'the position',
  'the role',
  'what you ll do',
  'who we are',
  'you ll do',
  'your role',
  'your tasks',
]);

/**
 * Reject a "company name" that is obviously a sentence, a heading, or markup
 * rather than an employer — returning `''`, which every downstream consumer
 * already treats as "no company known" (`CompanyResearch::enrich_with` skips
 * research entirely on an empty name).
 *
 * This runs on BOTH sources of the field: the model's own JSON and the regex
 * fallback in `extractMetadata`. Neither is trustworthy — a 2026-08-08 support
 * bundle had six consecutive generations research `You'll Do`,
 * `experience **fast`, and `a **bootstrapped AI platform and consulting
 * studio** based in Munich`, each of which then anchored a cover letter.
 *
 * Deliberately conservative: it only rejects shapes a real employer name cannot
 * have. Lowercase-initial brands (`eBay`, `iRobot`, `xAI`) and possessives
 * (`Macy's`, `L'Oréal`) survive; the leading-lowercase test requires a
 * lowercase SECOND character too, and the contraction test matches only
 * pronoun contractions.
 */
export function sanitizeCompanyName(value: unknown): string {
  return sanitizeSubject(value);
}

/**
 * The shared gate behind [`sanitizeCompanyName`] and [`sanitizeJobTitle`].
 *
 * Both fields end up as the SUBJECT of a provider web search and as prose in the
 * cover letter's opening, so both fail the same way: a heading or a sentence
 * there produces a confident brief about the wrong thing. Gating only the
 * company left the identical hole open for the title — a reported session
 * searched for `Jetzt bewerben` and `[← Alle offenen Stellen](/karriere)` as the
 * ROLE while the company was already being gated.
 */
function sanitizeSubject(value: unknown): string {
  if (typeof value !== 'string') return '';
  const name = value.trim();
  if (!name) return '';

  // A real employer name is short. The longest legitimate one seen in the wild
  // ("CHECK24 Vergleichsportal für Versicherungen GmbH") is 48 chars / 5 words.
  if (name.length > 60) return '';
  if (name.split(/\s+/).length > 6) return '';

  // Markdown, links, and code fences — the ad's formatting leaked through.
  if (/[*`_]{2}|\[|\]\(|https?:\/\//.test(name)) return '';

  // Prose, not a name. A trailing `.` is NOT disqualifying — "Acme Inc." is a
  // real name — but `!?:;` are, and so is an interior sentence break. The
  // `\w{3,}` before that break is what keeps abbreviations ("St. Jude Medical",
  // "A.P. Moller") from tripping it.
  if (/[!?:;]$/.test(name) || /\w{3,}[.!?][\s"']/.test(name)) return '';

  // Pronoun contractions only ever appear in ad copy ("What You'll Do",
  // "…their product. You'll sit at…"), never in a company name.
  if (/\b(?:you|we|they|i)['’](?:ll|re|ve|d|m)\b/i.test(name)) return '';

  // Mid-sentence fragment: starts lowercase AND continues lowercase.
  if (/^\p{Ll}\p{Ll}/u.test(name)) return '';
  // A single lowercase word ("experience", "je") is a fragment too.
  if (/^\p{Ll}(?:\s|$)/u.test(name)) return '';

  const normalized = name.toLowerCase().replace(/['’´]/g, ' ').replace(/\s+/g, ' ').trim();
  if (HEADING_DENYLIST.has(normalized)) return '';

  return name;
}

/**
 * A job title, gated exactly like the company name — see [`sanitizeSubject`].
 * Slightly more permissive on length: titles are legitimately longer than
 * company names ("Senior Staff Software Engineer, Payments Platform").
 */
export function sanitizeJobTitle(value: unknown): string {
  if (typeof value !== 'string') return '';
  if (value.trim().length > 80) return '';
  return sanitizeSubject(value);
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
      // Gated for the same reason `companyName` is: it is the other half of the
      // subject a provider web search runs on.
      jobTitle: sanitizeJobTitle(parsed.jobTitle),
      // Gated, not trusted: the model returns an ad heading ("You'll Do") as the
      // employer often enough that an ungated value reaches company research and
      // the cover letter's opening line.
      companyName: sanitizeCompanyName(parsed.companyName),
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
