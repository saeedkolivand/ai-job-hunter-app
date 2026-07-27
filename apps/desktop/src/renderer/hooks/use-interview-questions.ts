import { useMemo, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { detectLanguage, type InterviewQuestion } from '@ajh/shared';

import { useSelectedProvider } from '@/components/ui/ModelSelector';
import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import {
  extractMetadata,
  generateInterviewQuestions,
  type GenerationMeta,
  OUTPUT_LANGUAGES,
  parseInterviewQuestions,
  researchCompany as fetchCompanyBrief,
} from '@/lib/generate';
import { useAppClient } from '@/providers/AppClientProvider';
import { useHasProviderKey } from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';

/** Default target interviewers — the two earliest rounds (recruiter/HR + hiring manager). */
const DEFAULT_AUDIENCES = ['recruiter', 'hiringManager'];

/**
 * Normalize a detected language to a locale CODE. Accepts a code ('de') OR an
 * English language NAME ('German') — `extractMetadata`'s regex fallback can
 * produce either. `'unknown'` (detectLanguage's sentinel) and blanks become `''`,
 * meaning "not determined": the picker then shows its auto option and the prompt
 * falls back to its `meta`-derived language note.
 *
 * Deliberately NOT clamped to OUTPUT_LANGUAGES — a language the picker doesn't
 * offer (Dutch, Polish, Czech…) must survive to both the prompt and the picker
 * label, or the UI would read "English" while generating Dutch.
 */
function toLanguageCode(value: string): string {
  const trimmed = value.trim();
  if (!trimmed || trimmed === 'unknown') return '';
  const byName = OUTPUT_LANGUAGES.find(
    (l) => l.englishName.toLowerCase() === trimmed.toLowerCase()
  );
  return byName?.code ?? trimmed;
}

interface Params {
  resume: string;
  jobDesc: string;
  model: string;
  /** Reuse already-detected metadata when available — skips a re-extract. */
  meta?: GenerationMeta | null;
  canUse: boolean;
  hasDesc: boolean;
  /** Links the questions to the per-job application record (merge-upsert by url). */
  jobUrl: string;
  board: string;
}

/**
 * Generates AI-suggested "questions to ask the interviewer" for one job. Hybrid:
 * the model writes the questions and the user can bias them with seed topics.
 *
 * Unlike the cover-letter / application-answer flows, company research is ALWAYS
 * gathered here (not gated on a toggle) — good interview questions need concrete,
 * current company/role intel. The brief is fenced as untrusted by the prompt
 * layer (ADR-010). Detects metadata once (reusing the tailor flow's when given),
 * generates the delimited list, parses it leniently, then persists onto the
 * per-job aiGenerations aggregate (merge-upsert by `jobUrl`).
 *
 * The output `language` defaults to the ad's language (from `meta` when given,
 * else detected from `jobDesc`) and is user-overridable via `setLanguage`. It is
 * generation-only: the persisted record keeps the DETECTED `targetLanguage`,
 * because that field is shared with the email/export/tailor surfaces.
 */
export function useInterviewQuestions({
  resume,
  jobDesc,
  model,
  meta,
  canUse,
  hasDesc,
  jobUrl,
  board,
}: Params) {
  const api = useAppClient();
  const qc = useQueryClient();
  const [seedTopics, setSeedTopics] = useState('');
  // Target interviewers to generate for. Defaults to the two earliest rounds; the
  // user narrows or widens via the audience selector. Questions are generated PER
  // selected audience, tuned to that interviewer's lens.
  const [audiences, setAudiences] = useState<string[]>(DEFAULT_AUDIENCES);
  const [questions, setQuestions] = useState<InterviewQuestion[]>([]);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Output language. Derived during render (no effect) so the default keeps
  // tracking the ad as `jobDesc`/`meta` load in; the user's explicit pick then
  // wins for good. ONE effective value — what the picker shows IS what generates
  // (the UI adds an option for `detectedLanguage` when it isn't one of the
  // picker's own, so a Dutch ad reads as "Dutch", never as "English").
  // `''` means "not determined": the prompt then falls back to its `meta` note.
  const [languageOverride, setLanguageOverride] = useState<string | null>(null);
  // franc is not free, and this hook re-renders on every seed-topics keystroke.
  const detectedLanguage = useMemo(
    () => toLanguageCode(meta?.targetLanguage ?? '') || toLanguageCode(detectLanguage(jobDesc)),
    [meta?.targetLanguage, jobDesc]
  );
  const language = languageOverride ?? detectedLanguage;
  const generationLanguage = language || undefined;

  const toggleAudience = (aud: string) =>
    setAudiences((prev) => (prev.includes(aud) ? prev.filter((a) => a !== aud) : [...prev, aud]));

  // Company research ALWAYS runs here, via the active provider's web search.
  // Ollama-family providers need the free Ollama account key for it — surface a
  // non-blocking hint so the user knows their questions won't be web-grounded
  // until they add the key (Settings → AI). Does not gate generation.
  const activeProvider = useSelectedProvider();
  const { data: ollamaKey } = useHasProviderKey('ollama-cloud');
  const needsResearchKey =
    isOllamaFamily(activeProvider as AiProvider) && !(ollamaKey?.has ?? false);

  const canGenerate = canUse && hasDesc && resume.trim().length > 0 && audiences.length > 0;

  const generate = async () => {
    if (!canGenerate || generating) return;
    setGenerating(true);
    setError(null);
    try {
      const detected = meta ?? (await extractMetadata(resume, jobDesc, model));
      // Always gather company/role research — interview questions are only as good
      // as the intel behind them (server-cached, so it dedupes with other flows).
      const brief = await fetchCompanyBrief(jobDesc, detected.companyName);
      const seeds = seedTopics
        .split(/[\n,]/)
        .map((s) => s.trim())
        .filter(Boolean);
      const raw = await generateInterviewQuestions({
        resume,
        jobAd: jobDesc,
        meta: detected,
        model,
        companyBrief: brief,
        seedTopics: seeds,
        audiences,
        language: generationLanguage,
      });
      const parsed = parseInterviewQuestions(raw);
      setQuestions(parsed);

      // Persist onto the per-job application record (merge-upsert by jobUrl), so the
      // questions live alongside the résumé/cover/answers the tailor flow saved.
      await api.aiGenerations.save({
        candidateName: detected.candidateName,
        jobTitle: detected.jobTitle,
        companyName: detected.companyName,
        resumeLanguage: detected.resumeLanguage,
        jobAdLanguage: detected.jobAdLanguage,
        // NOT the picked language. `targetLanguage` is a SHARED field on the
        // per-job aggregate (the Rust merge is a non-blank-wins `pick`), read by
        // the email tab, the export meta and the tailor seed — writing the
        // questions' output language here would silently switch the email draft
        // and the export header. The pick stays generation-only.
        targetLanguage: detected.targetLanguage,
        mismatch: detected.mismatch,
        topRequirements: detected.topRequirements,
        mode: 'ats',
        resumeText: '',
        coverLetterText: '',
        jobAd: jobDesc,
        jobUrl,
        board,
        interviewQuestions: parsed,
        companyBrief: brief,
      });
      void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
      void qc.invalidateQueries({ queryKey: keys.autopilot.all });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate interview questions');
    } finally {
      setGenerating(false);
    }
  };

  return {
    seedTopics,
    setSeedTopics,
    language,
    /** The ad's own language (`''` when undetermined) — the UI offers it as an
     *  option when the picker's list doesn't already contain it. */
    detectedLanguage,
    setLanguage: setLanguageOverride,
    audiences,
    toggleAudience,
    questions,
    generating,
    error,
    generate,
    canGenerate,
    needsResearchKey,
  };
}
