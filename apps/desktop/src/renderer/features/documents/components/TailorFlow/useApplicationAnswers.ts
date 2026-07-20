import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { APPLICATION_QUESTIONS, countryToCurrency } from '@ajh/prompts/generate';
import type { ApplicationAnswer } from '@ajh/shared';

import {
  extractMetadata,
  generateApplicationAnswer,
  type GenerationMeta,
  lookupSalaryRange,
  researchAnswer as fetchAnswerWebNotes,
  researchCompany as fetchCompanyBrief,
  type SalaryRange,
} from '@/lib/generate';
import { useAppClient } from '@/providers/AppClientProvider';
import { keys } from '@/services/query-client';

/** Max length for a user-typed custom application question (chars, post-trim). */
export const MAX_CUSTOM_QUESTION_LEN = 500;

/**
 * Cap on how many selected questions can trigger a web search in a single
 * `generate()` run. `ai_research_answer` shares its per-provider daily budget
 * counter (`PROVIDER_DAILY_MAX`) with `ai_research_company`/`ai_lookup_salary`
 * — an uncapped fan-out over a 10-20 question form could dominate that shared
 * budget for the rest of the day. Anything past the cap still generates an
 * answer, just without web grounding (graceful — `webSearchNotes: ''`,
 * identical to the toggle being off for that question).
 *
 * Fuller alternative (deferred): a dedicated daily budget bucket just for
 * answer web-search, so it can never compete with salary/company research at
 * all.
 */
export const WEB_SEARCH_MAX_PER_RUN = 8;

interface Params {
  resume: string;
  jobDesc: string;
  model: string;
  /** Reuse the same opt-in research toggle as the cover letter (shared brief). */
  researchCompany: boolean;
  /** Metadata already detected by the tailor flow — skips a re-extract when set. */
  meta?: GenerationMeta | null;
  canUse: boolean;
  hasDesc: boolean;
  /** Links the answers to the per-job application record (merge-upsert by url). */
  jobUrl: string;
  board: string;
  /** Scraped salary (Phase 1 — from the job posting/application record), when
   *  known. Takes precedence over the web lookup for the salary question: it's
   *  the employer's own stated figure for this exact posting, not a market
   *  estimate. See {@link CURRENCY_SHAPE_RE}. */
  salaryMin?: number;
  salaryMax?: number;
  salaryCurrency?: string;
}

/** ISO-4217-ish currency code shape guard — mirrors the prompt layer's own
 *  `buildSalaryRangeBlock` guard (`packages/prompts/.../emphasis.ts`). */
const CURRENCY_SHAPE_RE = /^[A-Za-z]{3,4}$/;

/**
 * Builds a `SalaryRange` from a scraped min/max/currency triple, or `undefined`
 * when any part is missing or malformed (partial data — e.g. an amount without
 * a currency, or a currency that fails the ISO-4217-ish shape guard — must fall
 * through to the web lookup, never render a bogus range).
 *
 * MUST stay a strict superset of `buildSalaryRangeBlock`'s guard
 * (`packages/prompts/.../emphasis.ts`) — that's the prompt layer's OWN
 * re-check on the same shape across the package boundary. If this guard were
 * looser, a range that passes here but fails there renders an EMPTY
 * `<salary_context>` while having already skipped the web lookup: silent
 * degradation, worse than not scraping at all (e.g. `salaryMin: 0` from an
 * "up to X" Adzuna posting). Rounds to integers to match the web path's
 * `SalaryRange` (Rust `u32`, always whole) — validated AFTER rounding, so a
 * raw `min` in (0, 0.5) (which would round to 0) is rejected here too,
 * instead of passing this guard and then blanking at the prompt layer.
 * Currency is upper-cased to match the prompt block's rendered case even
 * when a source ever supplies lowercase.
 */
function buildScrapedSalaryRange(
  min?: number,
  max?: number,
  currency?: string
): SalaryRange | undefined {
  if (min == null || max == null || !currency) return undefined;
  if (!Number.isFinite(min) || !Number.isFinite(max)) return undefined;
  if (!CURRENCY_SHAPE_RE.test(currency)) return undefined;
  const lo = Math.round(min);
  const hi = Math.round(max);
  if (lo <= 0 || hi <= 0 || lo > hi) return undefined;
  return { min: lo, max: hi, currency: currency.toUpperCase() };
}

/**
 * Drafts résumé-grounded answers to a user-selected set of application questions.
 * Detects metadata once (reusing the tailor flow's when available), fetches the
 * company brief once when research is on (server-cached, so it dedupes with the
 * cover letter's), then answers each selected question through the shared
 * grounded pipeline — sequentially, filling answers in as they complete.
 */
export function useApplicationAnswers({
  resume,
  jobDesc,
  model,
  researchCompany,
  meta,
  canUse,
  hasDesc,
  jobUrl,
  board,
  salaryMin,
  salaryMax,
  salaryCurrency,
}: Params) {
  const api = useAppClient();
  const qc = useQueryClient();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Opt-in, per-question web search — local to this modal (not the tailor
  // flow's shared "researchCompany" form field). Off by default; when on,
  // each selected question's answer generation first fetches web-search
  // reference notes for that question (degrading to '' on failure/an
  // unsupported provider, so the answer still generates exactly as with it
  // off), up to `WEB_SEARCH_MAX_PER_RUN` questions per run — see its doc
  // comment for why the fan-out is capped.
  const [searchWeb, setSearchWeb] = useState(false);
  // `guidance` is always undefined for a user-typed custom question — declared
  // here (not cast later) so `chosen` below is a uniform shape and reading
  // `q.guidance` needs no narrowing/assertion for either branch.
  const [custom, setCustom] = useState<{ id: string; question: string; guidance?: string }[]>([]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Snapshot of the last successful generate() context — lets updateAnswer re-save
  // a single rewritten answer without re-running the whole pipeline.
  const lastSaveContextRef = useRef<{
    detected: GenerationMeta;
    brief: string;
  } | null>(null);
  // Mirror of `answers` state for stable reads inside async callbacks without stale
  // closures. Kept in sync by an effect — never mutated inside a setAnswers updater
  // (updaters must be pure: they may run twice in React 19 StrictMode).
  const answersRef = useRef<Record<string, string>>({});
  useEffect(() => {
    answersRef.current = answers;
  }, [answers]);

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const addCustom = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || trimmed.length > MAX_CUSTOM_QUESTION_LEN) return;
    setCustom((prev) => [...prev, { id: crypto.randomUUID(), question: trimmed }]);
  };

  const removeCustom = (id: string) => setCustom((prev) => prev.filter((c) => c.id !== id));

  const canGenerate =
    canUse && hasDesc && resume.trim().length > 0 && (selected.size > 0 || custom.length > 0);

  const saveAnswers = async (
    detected: GenerationMeta,
    brief: string,
    results: ApplicationAnswer[]
  ) => {
    await api.aiGenerations.save({
      candidateName: detected.candidateName,
      jobTitle: detected.jobTitle,
      companyName: detected.companyName,
      resumeLanguage: detected.resumeLanguage,
      jobAdLanguage: detected.jobAdLanguage,
      targetLanguage: detected.targetLanguage,
      mismatch: detected.mismatch,
      topRequirements: detected.topRequirements,
      mode: 'ats',
      resumeText: '',
      coverLetterText: '',
      jobAd: jobDesc,
      jobUrl,
      board,
      applicationAnswers: results,
      companyBrief: brief,
    });
    void qc.invalidateQueries({ queryKey: keys.aiGenerations.all });
    void qc.invalidateQueries({ queryKey: keys.autopilot.all });
  };

  const generate = async () => {
    if (!canGenerate || generating) return;
    setGenerating(true);
    setError(null);
    try {
      const detected = meta ?? (await extractMetadata(resume, jobDesc, model));
      const brief = researchCompany ? await fetchCompanyBrief(jobDesc, detected.companyName) : '';
      const chosen = [...APPLICATION_QUESTIONS.filter((q) => selected.has(q.id)), ...custom];
      if (searchWeb && chosen.length > WEB_SEARCH_MAX_PER_RUN) {
        // `console.warn` (not `.info`) — this repo's `no-console` lint rule
        // only allows `warn`/`error`.
        console.warn(
          `[useApplicationAnswers] web search capped at ${WEB_SEARCH_MAX_PER_RUN} of ${chosen.length} selected questions this run (shared daily provider budget guard); the rest still generate without web grounding.`
        );
      }
      const results: ApplicationAnswer[] = [];
      let webSearchesRun = 0;
      for (const q of chosen) {
        // Salary question only: precedence is scraped (this exact posting's own
        // stated figure) → web-researched market range → none (C1 fallback).
        // A scraped range only counts when it's COMPLETE and well-formed; a
        // partial/malformed one falls through to the web lookup below. Belt-
        // and-suspenders try/catch on top of `lookupSalaryRange`'s own — a
        // lookup failure must NEVER block or fail the rest of this loop, just
        // leave `salaryRange` undefined.
        let salaryRange: SalaryRange | undefined;
        if (q.id === 'salary') {
          const scrapedRange = buildScrapedSalaryRange(salaryMin, salaryMax, salaryCurrency);
          if (scrapedRange) {
            salaryRange = scrapedRange;
          } else {
            try {
              salaryRange = await lookupSalaryRange(
                detected.jobTitle,
                detected.companyName,
                detected.jobLocation || detected.jobCountry || '',
                detected.jobCountry,
                countryToCurrency(detected.jobCountry)
              );
            } catch {
              salaryRange = undefined;
            }
          }
        }
        // Opt-in per-question web search: fetch reference notes for THIS
        // question before answering it. `fetchAnswerWebNotes` already degrades
        // to '' on any failure or an unsupported provider; belt-and-suspenders
        // try/catch on top (mirrors the salary lookup above) so a search
        // failure can NEVER block or fail the rest of this loop — the answer
        // still generates exactly as with the toggle off. Capped at
        // `WEB_SEARCH_MAX_PER_RUN` per run (see its doc comment) — past the
        // cap, this question generates without web grounding.
        let webSearchNotes = '';
        if (searchWeb && webSearchesRun < WEB_SEARCH_MAX_PER_RUN) {
          webSearchesRun += 1;
          try {
            webSearchNotes = await fetchAnswerWebNotes(
              q.question,
              detected.jobTitle,
              detected.companyName
            );
          } catch {
            webSearchNotes = '';
          }
        }
        const answer = await generateApplicationAnswer({
          question: q.question,
          resume,
          jobAd: jobDesc,
          meta: detected,
          model,
          companyBrief: brief,
          webSearchNotes,
          // Only registry entries carry `guidance`; custom questions are
          // always `undefined` (see the `custom` state shape above).
          guidance: q.guidance,
          salaryRange,
        });
        results.push({ id: q.id, question: q.question, answer });
        setAnswers((prev) => ({ ...prev, [q.id]: answer }));
      }

      // Persist onto the per-job application record (merge-upsert by jobUrl), so
      // answers + brief live alongside the résumé/cover the tailor flow saved.
      await saveAnswers(detected, brief, results);
      lastSaveContextRef.current = { detected, brief };
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate answers');
    } finally {
      setGenerating(false);
    }
  };

  /**
   * Replace a single answer in local state WITHOUT persisting — used to revert an
   * optimistic update when the IPC save fails so the UI matches the stored truth.
   */
  const revertAnswer = (id: string, prev: string) => {
    setAnswers((current) => ({ ...current, [id]: prev }));
  };

  /**
   * Replace a single answer (from an AI rewrite) and re-persist the full answer
   * set through the same save path as generate(). No-op when no prior save context
   * exists (i.e. no generate has completed yet — the button is disabled in that case).
   * The caller is responsible for reverting via revertAnswer() if this rejects.
   */
  const updateAnswer = async (id: string, text: string) => {
    const ctx = lastSaveContextRef.current;
    if (!ctx) return;
    // Optimistic update — answersRef is synced by the effect after the render.
    setAnswers((prev) => ({ ...prev, [id]: text }));
    // Build the full answer list from the ref snapshot merged with the new value.
    // answersRef.current still holds the pre-update snapshot at this point (the
    // effect hasn't run yet), so we explicitly merge [id]: text on top.
    const allAnswers = Object.entries({ ...answersRef.current, [id]: text }).map(([qId, ans]) => {
      const q = APPLICATION_QUESTIONS.find((p) => p.id === qId) ?? custom.find((c) => c.id === qId);
      return { id: qId, question: q?.question ?? qId, answer: ans };
    });
    await saveAnswers(ctx.detected, ctx.brief, allAnswers);
  };

  return {
    selected,
    toggle,
    searchWeb,
    setSearchWeb,
    custom,
    addCustom,
    removeCustom,
    answers,
    generating,
    error,
    generate,
    canGenerate,
    updateAnswer,
    revertAnswer,
  };
}
