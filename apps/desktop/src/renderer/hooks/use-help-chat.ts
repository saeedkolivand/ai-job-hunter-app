import { useEffect, useRef, useState } from 'react';

import {
  buildHelpDataGlance,
  type HelpChatEntry,
  resolveHelpChatSizing,
} from '@ajh/prompts/generate';
import { HelpSearchEntrySchema, type HelpSearchResult } from '@ajh/shared/schemas';
import { useTranslation } from '@ajh/translations';

import { TRACKED_INTERACTION_TYPES } from '@/constants/interactions';
import { getSupportSections } from '@/features/support/support-data';
import { generateHelpAnswer } from '@/lib/generate';
import { buildProviderProfile } from '@/lib/generate/provider-context';
import {
  useApplications,
  useAutopilots,
  useEmbeddingStatus,
  useHelpSearch,
  useInteractions,
} from '@/services';

/** One entry of the shipped corpus, with the id `help:search` ranks by. */
interface CorpusEntry extends HelpChatEntry {
  id: string;
}

/** A single chat turn. Session-only — nothing here is ever persisted. */
export interface HelpChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  /** Assistant turns only: the help entries the answer was grounded in. */
  sources?: ReadonlyArray<{ id: string; title: string }>;
  /**
   * Assistant turns only: how the entries were retrieved. `'keyword'` means the
   * dense arm did not run, which the UI says out loud rather than presenting a
   * lexical list as semantic.
   */
  mode?: HelpSearchResult['mode'];
}

interface Params {
  /** The active model id (`useSelectedModel`). */
  model: string;
  /** Whether AI is usable at all (`useCanUseAI().canUse`). */
  canUse: boolean;
}

// The per-field caps are read off the Zod schema rather than re-typed here: a
// help answer that grew past `body`'s cap would otherwise fail validation at the
// IPC boundary and break the WHOLE chat with an opaque error, and a second copy
// of the number is exactly how that goes unnoticed. Every shipped entry is
// comfortably inside these today; this is the guard for the one that isn't.
const TITLE_MAX = HelpSearchEntrySchema.shape.title.maxLength ?? 200;
const BODY_MAX = HelpSearchEntrySchema.shape.body.maxLength ?? 2000;

/** Flatten the shipped help corpus into the wire shape, one entry per FAQ item. */
function buildCorpus(t: (key: string) => string): CorpusEntry[] {
  return getSupportSections(t).flatMap((section) =>
    section.problems.map((problem) => ({
      // `Problem.id` IS the translation leaf id — the stable identifier the
      // reply comes back keyed by. The reply carries ids only, so this is the
      // only thing that maps a result back to its text.
      id: problem.id,
      title: problem.q.slice(0, TITLE_MAX),
      body: problem.a.slice(0, BODY_MAX),
    }))
  );
}

/**
 * The in-app help chat (ADR-043) — ask a question, get an answer grounded in
 * the app's own shipped help corpus plus a read-only glance at the user's data.
 *
 * Entirely SESSION-ONLY, deliberately: nothing here persists (no IPC save, no
 * migration, no query cache), so closing the Help page discards the transcript.
 * That is the same shape as {@link useInterviewPractice}, and it is what keeps
 * a user's typed questions from becoming stored data.
 *
 * Two backend calls per question, in order: `help:search` ranks the corpus
 * (Rust does the retrieval math — the renderer only supplies the ACTIVE
 * locale's entries), then `generateHelpAnswer` streams the answer from the
 * top entries. A retrieval failure surfaces as an error rather than silently
 * asking the model to answer from nothing.
 *
 * `model`/`canUse` are parameters rather than hooks read here so this file stays
 * out of the component layer, matching `useInterviewPractice`'s signature.
 */
export function useHelpChat({ model, canUse }: Params) {
  const { t, i18n } = useTranslation();
  const search = useHelpSearch();
  const { data: embeddingStatus } = useEmbeddingStatus();
  const { data: interactions = [] } = useInteractions();
  const { data: applications = [] } = useApplications();
  const { data: autopilots = [] } = useAutopilots();

  const [turns, setTurns] = useState<HelpChatMessage[]>([]);
  const [answer, setAnswer] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const abortRef = useRef<AbortController | null>(null);
  // Mirrors `answer` so `stop()` can commit the partial text without reading
  // state inside a setState updater (which React may invoke twice).
  const answerRef = useRef('');
  // What the in-flight answer is grounded in — known as soon as retrieval
  // settles, so a Stop mid-stream can still attribute the partial answer.
  const pendingRef = useRef<Pick<HelpChatMessage, 'sources' | 'mode'>>({});
  // Monotonic per-question nonce: turn ids must never collide across a session,
  // and a positional index would repeat after a Stop discards nothing.
  const nonceRef = useRef(0);

  // Abort an in-flight stream on unmount — the Help page is a route, so
  // navigating away is the common case, not the exception.
  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    []
  );

  /** Stop the current stream, keeping whatever text already arrived. */
  const stop = () => {
    const controller = abortRef.current;
    if (!controller) return;
    controller.abort();
    abortRef.current = null;
    const partial = answerRef.current;
    answerRef.current = '';
    setAnswer('');
    setStreaming(false);
    // A user who presses Stop wants the half-written answer, not an empty card.
    if (partial.trim()) {
      nonceRef.current += 1;
      setTurns((prev) => [
        ...prev,
        {
          id: `a-${nonceRef.current}`,
          role: 'assistant',
          content: partial,
          ...pendingRef.current,
        },
      ]);
    }
  };

  const send = async (question: string) => {
    const query = question.trim();
    if (!canUse || !query || streaming) return;

    const profile = buildProviderProfile(model);
    const sizing = resolveHelpChatSizing(profile);
    const corpus = buildCorpus(t);
    // The transcript BEFORE this question — the model gets continuity without
    // being handed the question twice.
    const history = turns.slice(-sizing.historyTurns).map((turn) => ({
      role: turn.role,
      content: turn.content,
    }));

    const controller = new AbortController();
    abortRef.current = controller;
    answerRef.current = '';
    pendingRef.current = {};
    nonceRef.current += 1;
    const nonce = nonceRef.current;

    setTurns((prev) => [...prev, { id: `q-${nonce}`, role: 'user', content: query }]);
    setAnswer('');
    setError(null);
    setStreaming(true);

    try {
      const result = await search.mutateAsync({
        query,
        entries: corpus,
        // The SAME budget the prompt builder will apply: asking for more
        // entries than the prompt can carry pays to embed text it then drops.
        limit: sizing.maxEntries,
      });
      if (controller.signal.aborted) return;

      const byId = new Map(corpus.map((entry) => [entry.id, entry]));
      const used = result.results
        .map((hit) => byId.get(hit.id))
        .filter((entry): entry is CorpusEntry => entry !== undefined);
      pendingRef.current = {
        sources: used.map((entry) => ({ id: entry.id, title: entry.title })),
        mode: result.mode,
      };

      const raw = await generateHelpAnswer({
        question: query,
        entries: used.map(({ title, body }) => ({ title, body })),
        dataGlance: buildHelpDataGlance({
          documentCount: embeddingStatus?.documents?.total ?? 0,
          interactionCounts: countTrackedInteractions(interactions),
          applicationsByStatus: countByStatus(applications),
          recentApplications: recentApplications(applications),
          autopilotCount: autopilots.length,
          target: profile,
        }),
        history,
        model,
        language: i18n.language,
        signal: controller.signal,
        onToken: (token) => {
          // A Stop (or a newer question) may already have cancelled this
          // stream — drop a late token instead of writing it into a
          // transcript that has moved on.
          if (controller.signal.aborted) return;
          answerRef.current += token;
          setAnswer(answerRef.current);
        },
      });
      if (controller.signal.aborted) return;

      answerRef.current = '';
      setAnswer('');
      setTurns((prev) => [
        ...prev,
        { id: `a-${nonce}`, role: 'assistant', content: raw, ...pendingRef.current },
      ]);
    } catch (err) {
      if (controller.signal.aborted) return;
      answerRef.current = '';
      setAnswer('');
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (abortRef.current === controller) {
        abortRef.current = null;
        setStreaming(false);
      }
    }
  };

  return { turns, answer, streaming, error, send, stop };
}

/** Counts keyed by tracked interaction type; untracked types are excluded. */
function countTrackedInteractions(
  interactions: ReadonlyArray<{ interactionType?: string }>
): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const interaction of interactions) {
    const type = interaction.interactionType ?? '';
    if (!TRACKED_INTERACTION_TYPES.has(type)) continue;
    counts[type] = (counts[type] ?? 0) + 1;
  }
  return counts;
}

function countByStatus(applications: ReadonlyArray<{ status?: string }>): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const application of applications) {
    const status = application.status ?? 'unknown';
    counts[status] = (counts[status] ?? 0) + 1;
  }
  return counts;
}

/** The 10 most recently touched applications, newest first. */
function recentApplications(
  applications: ReadonlyArray<{
    title?: string;
    company?: string;
    status?: string;
    updatedAt?: number;
  }>
): Array<{ title: string; company: string; status: string }> {
  return [...applications]
    .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0))
    .slice(0, 10)
    .map((application) => ({
      title: application.title ?? '',
      company: application.company ?? '',
      status: application.status ?? 'unknown',
    }));
}
