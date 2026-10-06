import {
  buildMetadataPrompt,
  type GenerationMeta,
  sanitizeCompanyName,
  sanitizeJobTitle,
  validateMetadata,
} from '@ajh/prompts/generate';
import { detectLanguages } from '@ajh/shared/language-detection';

import { errorClass } from '../../error-class';
import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

export async function extractMetadata(
  resume: string,
  jobAd: string,
  model: string,
  locale = 'en'
): Promise<GenerationMeta> {
  // Detect languages client-side
  const clientSideDetection = detectLanguages(resume, jobAd);

  const profile = buildProviderProfile(model);

  const { system, user } = buildMetadataPrompt(resume, jobAd, profile);
  try {
    // Analysis carries its own per-model temperature override plus the
    // `deterministic` intent — exact, non-creative output.
    const raw = await streamGenerate(model, system, user, 'analysis', 'deterministic', { locale });
    const meta = validateMetadata(raw);
    if (meta) {
      // `targetLanguage` is deliberately NOT overridden here — `validateMetadata`
      // already sets it to the model's own `jobAdLanguage`, i.e. the AD's
      // language, which is the correct target. The heuristic fallback below
      // agrees: both paths seed it from the job ad, never the source résumé.
      return {
        ...meta,
        resumeLanguage: clientSideDetection.resumeName,
        jobAdLanguage: clientSideDetection.jobAdName,
        mismatch: clientSideDetection.mismatch,
      };
    }
    console.warn('[extractMetadata] model returned unparseable JSON — using heuristics', {
      model,
      rawLength: raw.length,
    });
  } catch (err) {
    // Never silent: the heuristics below are materially worse than the model,
    // and a run that quietly took this path produced cover letters addressed to
    // a job-ad heading. Which path ran has to be visible in the log.
    console.warn('[extractMetadata] extraction failed — using heuristics', {
      model,
      error: errorClass(err),
    });
  }

  const nameMatch = resume.match(/^([A-Z][a-z]+ [A-Z][a-z]+(?:\s[A-Z][a-z]+)?)/m);
  // `\b` on every word alternative. Without it `at` matched inside "Wh(at) You'll
  // Do" / "gre(at) experience" / Dutch "d(at) je zelf", and `job` inside "jobs",
  // so the capture ran from mid-word to the next comma or newline.
  const titleMatch = jobAd.match(/\b(?:position|role|title|job)[:\s]+([^\n]+)/i);
  const companyMatch = jobAd.match(/(?:\b(?:at|company|employer|firm)|@)[:\s]+([^\n,]+)/i);
  return {
    candidateName: nameMatch?.[1] ?? '',
    // Same gate the model's own output goes through — this regex is the WORSE
    // of the two sources, so it certainly does not get to skip validation.
    jobTitle: sanitizeJobTitle(titleMatch?.[1]),
    companyName: sanitizeCompanyName(companyMatch?.[1]),
    resumeLanguage: clientSideDetection.resumeName,
    jobAdLanguage: clientSideDetection.jobAdName,
    mismatch: clientSideDetection.mismatch,
    // The target is who we're writing FOR — the job ad's language — not the
    // language the candidate's existing résumé happens to be written in (seeding
    // from `resumeName` pinned an English résumé applying to a German ad to
    // English, and `useTailorPipeline` would then PREFER that value on later runs).
    //
    // `.jobAd` (the ISO 639-1 CODE), never `.jobAdName` (the display NAME):
    // `targetLanguage` is persisted verbatim to `ai_generations.target_language`
    // and read back by Rust's `normalize_language` (`validate/content/mod.rs`),
    // which takes the first two alphanumeric chars — `'German'` becomes `'ge'`,
    // matching no language arm and silently disabling every per-language check.
    // `resumeLanguage`/`jobAdLanguage` stay NAMES on purpose (consumers read them
    // through `toLanguageCode`).
    targetLanguage: clientSideDetection.jobAd,
    topRequirements: [],
  };
}
