import { z } from 'zod';

/**
 * One help-corpus entry, sent by the renderer with every `help:search` call.
 *
 * The corpus is NOT duplicated into Rust: the 51 entries live in the
 * translation bundles (`support.faq.<section>Questions.<id>.{q,a}`) and are
 * rendered by `features/support/support-data.ts`, so the renderer ships the
 * ACTIVE locale's entries per request and Rust does only the retrieval math.
 * `id` is the translation leaf path (e.g. `jobScrapingQuestions.linkedinNoResults`)
 * — the reply carries ids only, never a copy of the text the caller already holds.
 */
export const HelpSearchEntrySchema = z.object({
  id: z
    .string()
    .min(1)
    .max(64)
    .regex(/^[A-Za-z0-9_.-]+$/),
  /** The entry's question (`q`) — the strongest topical signal, so it is the BM25 `title` column. */
  title: z.string().min(1).max(200),
  /** The entry's answer (`a`) — the BM25 `description` column, and the text the dense arm embeds. */
  body: z.string().min(1).max(2000),
});

/**
 * Request for `help:search` — rank renderer-supplied help entries against a
 * user question. See `commands::help` (Rust) for the lexical/dense/fusion
 * pipeline this drives; it reuses the same L1 `retrieval` module as
 * `scrape:hybridSearch` (ADR-039).
 *
 * Every cap here is re-checked server-side: a Tauri command is an IPC boundary
 * a non-UI caller (the agent CLI, a crafted extension message) reaches
 * directly, bypassing Zod entirely.
 */
export const HelpSearchRequestSchema = z.object({
  /**
   * Optional client-minted id for this question — MUST start with `"help-"`
   * (e.g. `` `help-${crypto.randomUUID()}` ``). Pass the SAME value to
   * `jobs.cancel(queryId)` to stop the dense arm early (a Stop click, or
   * navigating away): `commands::help` registers it against the app-wide
   * `jobs::cancel::CancelRegistry` exactly as `scrape:hybridSearch` does, so
   * no separate cancel channel exists. The prefix keeps this caller-minted id
   * space disjoint from both the Rust-minted `job-`/`run-` ids and the postings
   * search's `search-` ids that share the registry; the command re-checks it
   * (`commands::help::QUERY_ID_PREFIX`). Omitted = not cancellable, which is
   * what an agent-CLI caller gets.
   */
  queryId: z.string().min(1).max(64).startsWith('help-').optional(),
  /**
   * BCP-47 tag of the locale the `entries` are written in (the renderer's
   * active UI locale). Selects the function-word list the lexical arm drops
   * from the question before the OR-join (`commands::help::stopwords`); a
   * locale with no list drops nothing.
   *
   * Omitted = NO drop list, never English. A caller that does not say what
   * language its entries are in has not said "English", and filtering a
   * foreign corpus through English function words can only cost recall —
   * `in`, `an` and `es` are content words in other languages.
   */
  locale: z.string().min(2).max(16).optional(),
  /** The user's question, verbatim. Trimmed and re-capped by `commands::help`. */
  query: z.string().min(1).max(500),
  /**
   * The active locale's help corpus. Capped at 200 — comfortably above the
   * shipped 51 entries, so a hostile/buggy caller can't force an unbounded
   * embed pass (the dense arm embeds one entry per cache miss).
   */
  entries: z.array(HelpSearchEntrySchema).min(1).max(200),
  /** How many ranked entry ids to return, best first. */
  limit: z.number().int().min(1).max(10).default(3),
});

/**
 * Reply for `help:search`. `mode` is `'hybrid'` only when the dense arm
 * actually RAN — `arms` says exactly which of the two arms ran, so the UI can
 * say "keyword results; semantic ranking off" instead of presenting a lexical
 * list as hybrid. An embedding failure is never an error to the user: the
 * keyword results still come back with `dense: 'unavailable'`.
 */
export const HelpSearchResultSchema = z.object({
  /** At most `limit`, best first. `score` is the RRF fused score (see `retrieval::fusion`). */
  results: z.array(z.object({ id: z.string(), score: z.number() })),
  mode: z.enum(['hybrid', 'keyword']),
  arms: z.object({
    lexical: z.enum(['ran', 'unavailable']),
    dense: z.enum(['ran', 'skipped', 'unavailable']),
  }),
});

export type HelpSearchEntry = z.infer<typeof HelpSearchEntrySchema>;
export type HelpSearchRequest = z.infer<typeof HelpSearchRequestSchema>;
export type HelpSearchResult = z.infer<typeof HelpSearchResultSchema>;
