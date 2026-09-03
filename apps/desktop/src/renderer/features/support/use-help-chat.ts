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
import { useCancelJob, useFetchHelpDataSources, useHelpSearch } from '@/services';

/** One entry of the shipped corpus, with the id `help:search` ranks by. */
interface CorpusEntry extends HelpChatEntry {
  id: string;
  /** The `Section.id` this entry came from — local only, never on the wire. */
  section: string;
}

/**
 * The section whose entries are about the user's tracked applications
 * (`support.faq.applicationsQuestions.*`). Only a question that retrieved one
 * of those is answered any better by knowing WHICH jobs the user applied to —
 * see the glance below.
 */
const APPLICATIONS_SECTION = 'applications';

/**
 * Prefix every minted `queryId` carries. MUST match the Rust-side check in
 * `commands::help` on top of `HelpSearchRequestSchema`'s own `.startsWith` —
 * mirrored, not imported, exactly as `usePostingsSearch` mirrors its
 * `search-` prefix. The prefix is what keeps this caller-minted id space from
 * naming a live `job-`/`run-`/`search-` id in the shared cancel registry.
 * A UUID v4 is 36 chars, so the prefixed id stays well under the 64-char cap.
 */
const QUERY_ID_PREFIX = 'help-';

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
  /**
   * Assistant turns only: WHY the dense arm did not run. `'skipped'` is the
   * user's own opt-out, fixable in Settings; `'unavailable'` is an embedding
   * failure with the preference already ON, where there is nothing for the user
   * to switch. `mode` alone collapses the two into one message that is wrong
   * half the time, so the UI reads this instead.
   */
  dense?: HelpSearchResult['arms']['dense'];
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
      section: section.id,
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
 * The data glance is fetched PER QUESTION and never on mount, so opening Help
 * to read one entry does not read the user's applications — see
 * `useFetchHelpDataSources`.
 *
 * `model`/`canUse` are parameters rather than hooks read here so this file stays
 * out of the component layer, matching `useInterviewPractice`'s signature.
 *
 * It lives INSIDE `features/support/` rather than in `hooks/` because it reads
 * `features/support/support-data`: a shared-layer module importing a feature
 * inverts the dependency rule the feature dirs exist to enforce. Nothing
 * outside this feature references it.
 */
export function useHelpChat({ model, canUse }: Params) {
  const { t, i18n } = useTranslation();
  const search = useHelpSearch();
  const fetchDataSources = useFetchHelpDataSources();
  const cancelJob = useCancelJob();

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
  const pendingRef = useRef<Pick<HelpChatMessage, 'sources' | 'mode' | 'dense'>>({});
  // Monotonic per-question nonce: turn ids must never collide across a session,
  // and a positional index would repeat after a Stop discards nothing.
  const nonceRef = useRef(0);
  // Whether the in-flight question has reached its STREAM. `help_search` and the
  // data-glance reads are Tauri commands: the controller's signal cannot cancel
  // them, it can only make the renderer ignore the reply. What CAN stop the
  // retrieval leg is `jobs.cancel(queryId)` — see `cancelRetrieval` below.
  const streamPhaseRef = useRef(false);
  // The `queryId` of the retrieval leg that is still cancellable, or `null`.
  // Cleared the moment `help_search` settles: cancelling a completed id is a
  // no-op that still emits a `job.cancelled` event and invalidates the whole
  // jobs list, so the id must not outlive the leg it names.
  const queryIdRef = useRef<string | null>(null);
  // Latest cancel handle. `useCancelJob()` returns a fresh object each render,
  // so the unmount cleanup below (which MUST have an empty dep list — a
  // cleanup that re-ran every render would cancel the question in flight)
  // reads it through a ref instead of closing over it.
  const cancelJobRef = useRef(cancelJob);
  useEffect(() => {
    cancelJobRef.current = cancelJob;
  });

  /**
   * Stop the backend half of an in-flight question, if there is one.
   *
   * Best-effort and fire-and-forget, mirroring `usePostingsSearch`: the invoke
   * promise is not abortable, so this only makes the dense arm give up sooner
   * — the reply is already being ignored either way. A failed cancel is
   * nothing the user can act on and nothing they asked for, so it is swallowed
   * rather than turned into a toast about a request they never made.
   */
  const cancelRetrieval = () => {
    const queryId = queryIdRef.current;
    if (!queryId) return;
    queryIdRef.current = null;
    void cancelJobRef.current.mutateAsync(queryId).catch(() => {});
  };

  // Abort an in-flight stream on unmount, and cancel the retrieval leg behind
  // it — the Help page is a route, so navigating away is the common case, not
  // the exception, and the embedding pass it leaves behind is the expensive
  // half. Inlined rather than calling `cancelRetrieval`: the dep list has to
  // stay empty (see `cancelJobRef`).
  useEffect(
    () => () => {
      abortRef.current?.abort();
      const queryId = queryIdRef.current;
      if (queryId) void cancelJobRef.current.mutateAsync(queryId).catch(() => {});
    },
    []
  );

  /**
   * Stop the current stream, keeping whatever text already arrived.
   *
   * Busy-state release is deliberately conditional, and stays that way now
   * that the retrieval leg IS cancellable. During RETRIEVAL the cancel makes
   * the backend give up sooner, not instantly — it is checked between dense
   * candidates and raced against each embed, so an embed already in flight
   * still finishes. Clearing `streaming` here would hand the Ask button back
   * during that window and let a second question's run share the first run's
   * `finally`. So the abort is recorded (the reply is dropped), the cancel is
   * sent, and `run`'s `finally` releases the button when the leg actually
   * settles. Once the stream has started, aborting really does end the work,
   * so Ask comes back immediately as before.
   */
  const stop = () => {
    const controller = abortRef.current;
    if (!controller || controller.signal.aborted) return;
    controller.abort();
    if (!streamPhaseRef.current) cancelRetrieval();
    const partial = answerRef.current;
    answerRef.current = '';
    setAnswer('');
    if (streamPhaseRef.current) {
      abortRef.current = null;
      setStreaming(false);
    }
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

  /**
   * Answer one question end to end.
   *
   * `appendUserTurn` is false for a retry: the failed question is already in the
   * transcript, and repeating it there would make one retry look like two
   * questions. Resolves `true` only when an assistant turn actually landed —
   * that is what tells the caller it may clear the question box, so a question
   * that failed is never silently thrown away.
   *
   * NEVER rejects: everything after the controller is created runs inside the
   * try, so a caller can treat the boolean as the whole outcome and every
   * failure reaches the user through `error` instead of an unhandled rejection.
   */
  const run = async (question: string, appendUserTurn: boolean): Promise<boolean> => {
    const query = question.trim();
    if (!canUse || !query || streaming) return false;

    // Abort whatever this run replaces BEFORE overwriting the ref, exactly as
    // `useInterviewPractice` does. The `streaming` guard above closes the
    // rendered path, but two `send`s in the same tick both read the stale
    // `false`; without this the first controller is dropped un-aborted and its
    // stream keeps writing turns into a transcript that moved on.
    abortRef.current?.abort();
    // …and stop the replaced run's retrieval in the backend too, for the same
    // reason: the abort only makes the renderer ignore its reply.
    cancelRetrieval();

    const controller = new AbortController();
    abortRef.current = controller;
    streamPhaseRef.current = false;
    answerRef.current = '';
    pendingRef.current = {};
    nonceRef.current += 1;
    const nonce = nonceRef.current;
    const queryId = `${QUERY_ID_PREFIX}${crypto.randomUUID()}`;
    queryIdRef.current = queryId;

    setAnswer('');
    setError(null);
    setStreaming(true);

    try {
      const profile = buildProviderProfile(model);
      const sizing = resolveHelpChatSizing(profile);
      const corpus = buildCorpus(t);
      // The transcript BEFORE this question — the model gets continuity without
      // being handed the question twice. On a retry the failed user turn is
      // already the tail, so it is dropped here for exactly the same reason.
      const prior = appendUserTurn ? turns : turns.slice(0, -1);
      const history = prior.slice(-sizing.historyTurns).map((turn) => ({
        role: turn.role,
        content: turn.content,
      }));

      if (appendUserTurn) {
        setTurns((prev) => [...prev, { id: `q-${nonce}`, role: 'user', content: query }]);
      }

      const result = await search.mutateAsync({
        // Minted per question, so a Stop or an unmount can name THIS retrieval
        // to `jobs.cancel` — there is no separate cancel channel.
        queryId,
        // The locale the `entries` below are written in: it selects the
        // function-word list the lexical arm drops from the question. Sending
        // the UI language rather than a constant is what keeps a German
        // question from being filtered with an English list, or not at all.
        locale: i18n.language,
        query,
        // Only the fields the contract names: `section` is a local routing
        // hint, and sending a field the schema does not describe is how a wire
        // shape drifts away from it.
        entries: corpus.map(({ id, title, body }) => ({ id, title, body })),
        // The SAME budget the prompt builder will apply: asking for more
        // entries than the prompt can carry pays to embed text it then drops.
        limit: sizing.maxEntries,
      });
      // The retrieval leg is done, cancellable or not — drop the id so a later
      // Stop/unmount cannot fire a cancel for work that already finished.
      queryIdRef.current = null;
      if (controller.signal.aborted) return false;

      const byId = new Map(corpus.map((entry) => [entry.id, entry]));
      const used = result.results
        .map((hit) => byId.get(hit.id))
        .filter((entry): entry is CorpusEntry => entry !== undefined);
      pendingRef.current = {
        sources: used.map((entry) => ({ id: entry.id, title: entry.title })),
        mode: result.mode,
        dense: result.arms.dense,
      };

      // Read the user's own lists only now — a question was actually asked.
      // Any of the four may come back `null`: that source could not be read, and
      // the glance leaves its line out rather than claiming a zero.
      const [embeddingStatus, interactions, applications, autopilots] = await fetchDataSources();
      if (controller.signal.aborted) return false;

      // The recent-application list is the only part of the glance carrying the
      // user's job titles and company names, and the only part that leaves the
      // machine as prose. Counts answer "have I tracked anything at all"; the
      // NAMES only help a question that retrieved an applications entry, so
      // that is the only question that pays to send them to the provider.
      const aboutApplications = used.some((entry) => entry.section === APPLICATIONS_SECTION);

      // Everything uncancellable is behind us; from here a Stop really stops
      // the work, so it may release the Ask button immediately.
      streamPhaseRef.current = true;
      const raw = await generateHelpAnswer({
        question: query,
        entries: used.map(({ title, body }) => ({ title, body })),
        dataGlance: buildHelpDataGlance({
          documentCount: embeddingStatus ? (embeddingStatus.documents?.total ?? 0) : null,
          interactionCounts: interactions ? countTrackedInteractions(interactions) : null,
          applicationsByStatus: applications ? countByStatus(applications) : null,
          recentApplications:
            aboutApplications && applications ? recentApplications(applications) : [],
          autopilotCount: autopilots ? autopilots.length : null,
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
      if (controller.signal.aborted) return false;

      answerRef.current = '';
      setAnswer('');
      setTurns((prev) => [
        ...prev,
        { id: `a-${nonce}`, role: 'assistant', content: raw, ...pendingRef.current },
      ]);
      return true;
    } catch (err) {
      if (controller.signal.aborted) return false;
      answerRef.current = '';
      setAnswer('');
      setError(err instanceof Error ? err.message : String(err));
      return false;
    } finally {
      // Still the current run — including one aborted DURING retrieval, whose
      // `stop()` deliberately left the ref in place so the busy state outlived
      // the abort. This is where that run finally gives the button back.
      if (abortRef.current === controller) {
        abortRef.current = null;
        streamPhaseRef.current = false;
        // Safety net for the path the clear after `search.mutateAsync` misses:
        // a retrieval that REJECTED never reached it.
        if (queryIdRef.current === queryId) queryIdRef.current = null;
        setStreaming(false);
      }
    }
  };

  /** Ask a new question: appends the user turn, then answers it. */
  const send = (question: string) => run(question, true);

  /**
   * Re-run the last question after a failure. The user turn is already on
   * screen, so this re-answers it in place instead of asking it twice.
   */
  const retry = () => {
    const last = [...turns].reverse().find((turn) => turn.role === 'user');
    return last ? run(last.content, false) : Promise.resolve(false);
  };

  return { turns, answer, streaming, error, send, retry, stop };
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
