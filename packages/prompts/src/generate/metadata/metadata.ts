/** Metadata-extraction prompt + validator. */

import { type PromptTarget, resolveProfile } from '../../provider/index.js';
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

<job_ad>
${jobAd.slice(0, 4000)}
</job_ad>

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
