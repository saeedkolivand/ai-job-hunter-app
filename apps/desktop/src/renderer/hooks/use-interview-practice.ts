import { useEffect, useRef, useState } from 'react';

import {
  extractMetadata,
  generateLikelyInterviewQuestions,
  generateStarFeedback,
  type GenerationMeta,
  type LikelyQuestion,
  parseLikelyQuestions,
  parseStarFeedback,
  type StarFeedback,
} from '@/lib/generate';

interface Params {
  resume: string;
  jobDesc: string;
  model: string;
  /** Reuse already-detected metadata when available — skips a re-extract. */
  meta?: GenerationMeta | null;
  canUse: boolean;
  hasDesc: boolean;
}

/** Per-question STAR-feedback request state, keyed by question id. */
interface FeedbackState {
  /** Raw streamed text so far — lets the panel show a live StreamingText. */
  text: string;
  /** Parsed rubric once the stream completes; null while streaming or errored. */
  feedback: StarFeedback | null;
  loading: boolean;
  error: string | null;
}

/**
 * Mock-interview practice: generates likely questions the CANDIDATE will be
 * asked for this job, then — per question — streams STAR-rubric feedback on a
 * typed practice answer. Mirrors {@link useInterviewQuestions}'s extract-meta ->
 * generate shape, but is entirely SESSION-ONLY: nothing here persists to the
 * per-job aiGenerations aggregate (no IPC save, no Rust/migration) — questions,
 * answers, and feedback are lost when the tab unmounts.
 */
export function useInterviewPractice({ resume, jobDesc, model, meta, canUse, hasDesc }: Params) {
  const [questions, setQuestions] = useState<LikelyQuestion[]>([]);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<Record<string, FeedbackState>>({});
  // The metadata used to generate the current question set — reused for every
  // feedback request so it doesn't re-extract per question.
  const detectedMetaRef = useRef<GenerationMeta | null>(meta ?? null);
  const abortRefs = useRef<Record<string, AbortController>>({});
  // Monotonic per-generation nonce stamped onto every question id (see
  // `generate` below) so a Regenerate can never collide with the previous
  // set's positional ids.
  const genCounterRef = useRef(0);

  const canGenerate = canUse && hasDesc && resume.trim().length > 0;

  // Abort every in-flight feedback stream on unmount.
  useEffect(
    () => () => {
      Object.values(abortRefs.current).forEach((c) => c.abort());
    },
    []
  );

  const generate = async () => {
    if (!canGenerate || generating) return;
    // Abort every outstanding feedback stream BEFORE starting a new generation
    // — a stale STAR request in flight from the previous question set must
    // never resolve/stream into state after `setFeedback({})` below.
    Object.values(abortRefs.current).forEach((c) => c.abort());
    abortRefs.current = {};
    setGenerating(true);
    setError(null);
    try {
      const detected = meta ?? (await extractMetadata(resume, jobDesc, model));
      detectedMetaRef.current = detected;
      const raw = await generateLikelyInterviewQuestions({
        resume,
        jobAd: jobDesc,
        meta: detected,
        model,
      });
      // Stamp a per-generation nonce onto every id. `parseLikelyQuestions`
      // numbers each block positionally (`lq-1`, `lq-2`…) WITHIN one parse call
      // only, so without this a Regenerate would reuse the previous set's ids
      // — silently reattaching a stale typed answer or feedback entry (keyed by
      // id) onto the wrong, freshly-generated question.
      genCounterRef.current += 1;
      const gen = genCounterRef.current;
      setQuestions(parseLikelyQuestions(raw).map((q) => ({ ...q, id: `${gen}-${q.id}` })));
      setFeedback({});
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to generate practice questions');
    } finally {
      setGenerating(false);
    }
  };

  /** Request STAR feedback for one question + the candidate's typed answer.
   *  No-ops when AI is unavailable, the answer is blank, or no question set has
   *  been generated yet (no metadata to ground the feedback in). */
  const getFeedback = async (question: LikelyQuestion, answer: string) => {
    const trimmed = answer.trim();
    const detected = detectedMetaRef.current;
    if (!canUse || !trimmed || !detected) return;

    // Cancel a previous in-flight request for the same question before starting.
    abortRefs.current[question.id]?.abort();
    const controller = new AbortController();
    abortRefs.current[question.id] = controller;
    setFeedback((prev) => ({
      ...prev,
      [question.id]: { text: '', feedback: null, loading: true, error: null },
    }));

    try {
      const raw = await generateStarFeedback({
        question: question.question,
        answer: trimmed,
        resume,
        jobAd: jobDesc,
        meta: detected,
        model,
        signal: controller.signal,
        onToken: (tok) => {
          // A newer request for this question, or a Regenerate (which aborts
          // every outstanding controller — see `generate`), may have already
          // cancelled this stream. Drop a late token instead of writing it.
          if (controller.signal.aborted) return;
          setFeedback((prev) => {
            const cur = prev[question.id];
            return {
              ...prev,
              [question.id]: {
                text: (cur?.text ?? '') + tok,
                feedback: null,
                loading: true,
                error: null,
              },
            };
          });
        },
      });
      if (controller.signal.aborted) return;
      setFeedback((prev) => ({
        ...prev,
        [question.id]: { text: raw, feedback: parseStarFeedback(raw), loading: false, error: null },
      }));
    } catch (err) {
      if (!controller.signal.aborted) {
        setFeedback((prev) => ({
          ...prev,
          [question.id]: {
            text: '',
            feedback: null,
            loading: false,
            error: err instanceof Error ? err.message : 'Failed to get feedback',
          },
        }));
      }
    } finally {
      if (abortRefs.current[question.id] === controller) delete abortRefs.current[question.id];
    }
  };

  return { questions, generating, error, generate, canGenerate, feedback, getFeedback };
}
